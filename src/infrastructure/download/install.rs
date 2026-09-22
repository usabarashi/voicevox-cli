use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};

use super::{
    cleanup::{cleanup_incomplete_downloads, cleanup_unnecessary_files, count_vvm_files_recursive},
    find_downloader_binary,
};
use crate::infrastructure::paths::get_default_voicevox_dir;

/// Total downloader invocations the installer performs.
///
/// Production `max_retries = 3` means three total invocations (not an initial
/// attempt plus three retries). Mirrors `MAX_ATTEMPTS` in
/// `modeling/quint/Download.qnt`.
#[doc(hidden)]
pub const MAX_DOWNLOAD_ATTEMPTS: u32 = 3;

/// Phase of the installer retry loop. Mirrors `DState` in `Download.qnt`.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadPhase {
    Idle,
    Downloading,
    Cleaning,
    Done,
    Failed,
}

/// Terminal failure class. Mirrors `FailureKind` in `Download.qnt` (the
/// preparation-failure case is detected before the loop and is not produced by
/// the tracker).
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadFailure {
    NoFailure,
    Exhausted,
}

/// Retry state machine for the installer.
///
/// `install_with_retries` drives this for the real installer; the model-based
/// test `mbt/tests/download.rs` observes the resulting `(phase, attempts,
/// failure)` against `modeling/quint/Download.qnt`. Attempts are counted at
/// invocation start.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadTracker {
    max_attempts: u32,
    phase: DownloadPhase,
    attempts: u32,
    failure: DownloadFailure,
}

impl DownloadTracker {
    #[must_use]
    pub const fn new(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            phase: DownloadPhase::Idle,
            attempts: 0,
            failure: DownloadFailure::NoFailure,
        }
    }

    #[must_use]
    pub const fn phase(&self) -> DownloadPhase {
        self.phase
    }

    #[must_use]
    pub const fn attempts(&self) -> u32 {
        self.attempts
    }

    #[must_use]
    pub const fn failure(&self) -> DownloadFailure {
        self.failure
    }

    /// `Idle -> Downloading`, counting the first invocation.
    pub fn begin(&mut self) -> bool {
        if self.phase != DownloadPhase::Idle {
            return false;
        }
        self.phase = DownloadPhase::Downloading;
        self.attempts += 1;
        true
    }

    /// `Downloading -> Done`.
    pub fn succeeded(&mut self) -> bool {
        if self.phase != DownloadPhase::Downloading {
            return false;
        }
        self.phase = DownloadPhase::Done;
        true
    }

    /// A failed invocation either needs cleanup before a retry, or exhausts the
    /// invocation budget.
    pub fn failed(&mut self) -> bool {
        if self.phase != DownloadPhase::Downloading {
            return false;
        }
        if self.attempts >= self.max_attempts {
            self.phase = DownloadPhase::Failed;
            self.failure = DownloadFailure::Exhausted;
        } else {
            self.phase = DownloadPhase::Cleaning;
        }
        true
    }

    /// `Cleaning -> Downloading`, counting the next invocation.
    pub fn cleanup_done(&mut self) -> bool {
        if self.phase != DownloadPhase::Cleaning {
            return false;
        }
        self.phase = DownloadPhase::Downloading;
        self.attempts += 1;
        true
    }
}

/// Result of one downloader invocation.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadAttempt {
    Succeeded,
    Failed { detail: String },
}

/// The installer's environment: one downloader invocation, cleanup, and the
/// wait between invocations.
#[doc(hidden)]
#[allow(async_fn_in_trait)]
pub trait ResourceInstaller {
    async fn run_once(&mut self) -> DownloadAttempt;
    fn cleanup(&mut self);
    async fn wait_before_retry(&mut self);
}

/// Report of the retry loop (always returned; inspect `phase`).
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallReport {
    pub phase: DownloadPhase,
    pub attempts: u32,
    pub failure: DownloadFailure,
    pub last_detail: Option<String>,
}

/// Runs the installer with the retry/cleanup policy, driven by
/// [`DownloadTracker`].
#[doc(hidden)]
pub async fn install_with_retries<I: ResourceInstaller>(
    installer: &mut I,
    max_attempts: u32,
) -> InstallReport {
    let mut tracker = DownloadTracker::new(max_attempts);
    let mut last_detail = None;
    tracker.begin();

    loop {
        match installer.run_once().await {
            DownloadAttempt::Succeeded => {
                tracker.succeeded();
                break;
            }
            DownloadAttempt::Failed { detail } => {
                last_detail = Some(detail);
                installer.cleanup();
                tracker.failed();
                if tracker.phase() == DownloadPhase::Failed {
                    break;
                }
                tracker.cleanup_done();
                installer.wait_before_retry().await;
            }
        }
    }

    InstallReport {
        phase: tracker.phase(),
        attempts: tracker.attempts(),
        failure: tracker.failure(),
        last_detail,
    }
}

/// Production installer: spawns the external downloader and cleans the target
/// directory between invocations.
struct CommandInstaller {
    downloader_path: PathBuf,
    missing_resources: Vec<String>,
    target_dir: PathBuf,
}

impl CommandInstaller {
    fn new(downloader_path: PathBuf, missing_resources: Vec<String>, target_dir: PathBuf) -> Self {
        Self {
            downloader_path,
            missing_resources,
            target_dir,
        }
    }
}

impl ResourceInstaller for CommandInstaller {
    async fn run_once(&mut self) -> DownloadAttempt {
        let mut command = tokio::process::Command::new(&self.downloader_path);
        for resource in &self.missing_resources {
            command.arg("--only").arg(resource);
        }
        command.arg("--output").arg(&self.target_dir);

        match command.status().await {
            Ok(status) if status.success() => DownloadAttempt::Succeeded,
            Ok(status) => DownloadAttempt::Failed {
                detail: format!("Download failed with exit code: {:?}", status.code()),
            },
            Err(error) => DownloadAttempt::Failed {
                detail: format!("Failed to execute downloader: {error}"),
            },
        }
    }

    fn cleanup(&mut self) {
        cleanup_incomplete_downloads(&self.target_dir);
    }

    async fn wait_before_retry(&mut self) {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

pub fn missing_resource_descriptions(missing_resources: &[&str]) -> Vec<&'static str> {
    let mut descriptions = Vec::new();
    if missing_resources.contains(&"onnxruntime") {
        descriptions.push("ONNX Runtime - Neural network inference engine");
    }
    if missing_resources.contains(&"dict") {
        descriptions.push("OpenJTalk Dictionary - Japanese text processing");
    }
    if missing_resources.contains(&"models") {
        descriptions.push("Voice Models - Character voices");
    }
    descriptions
}

pub async fn download_missing_resources(missing_resources: &[&str]) -> Result<()> {
    if missing_resources.is_empty() {
        return Ok(());
    }

    let target_dir = get_default_voicevox_dir();
    tokio::fs::create_dir_all(&target_dir).await?;
    let downloader_path = find_downloader_binary()?;

    let mut installer = CommandInstaller::new(
        downloader_path,
        missing_resources.iter().map(|s| (*s).to_string()).collect(),
        target_dir.clone(),
    );
    let report = install_with_retries(&mut installer, MAX_DOWNLOAD_ATTEMPTS).await;

    if report.phase == DownloadPhase::Done {
        return Ok(());
    }

    // `install_with_retries` already ran `installer.cleanup()` on the terminal
    // failure; do not clean up a second time here.
    let details = report
        .last_detail
        .unwrap_or_else(|| "unknown error".to_string());
    Err(anyhow!(
        "Failed to download required resources after {MAX_DOWNLOAD_ATTEMPTS} attempts: {details}"
    ))
}

pub async fn launch_models_downloader(target_dir: &Path) -> Result<usize> {
    tokio::fs::create_dir_all(target_dir).await?;
    let downloader_path = find_downloader_binary()?;

    let status = tokio::process::Command::new(&downloader_path)
        .arg("--only")
        .arg("models")
        .arg("--output")
        .arg(target_dir)
        .status()
        .await?;

    if !status.success() {
        return Err(anyhow!("Download process failed or was cancelled"));
    }

    let vvm_count = count_vvm_files_recursive(target_dir);
    if vvm_count == 0 {
        return Err(anyhow!(
            "Download completed but voice model files were not found in target directory"
        ));
    }

    cleanup_unnecessary_files(target_dir);
    Ok(vvm_count)
}

pub fn default_models_download_target_dir() -> PathBuf {
    super::default_download_target_dir()
}
