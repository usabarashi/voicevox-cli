use std::path::Path;

pub(crate) fn cleanup_incomplete_downloads(target_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(target_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();

        if is_temporary_download_file(&path) {
            log_remove_file(&path, "temporary file", "Cleaned up temporary file");
            continue;
        }

        if file_type.is_file() && is_likely_incomplete_download_file(&path) {
            log_remove_file(&path, "incomplete file", "Cleaned up incomplete file");
        }
    }
}

fn is_temporary_download_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|ext| {
            ext.eq_ignore_ascii_case("tmp")
                || ext.eq_ignore_ascii_case("download")
                || ext.eq_ignore_ascii_case("partial")
        })
}

fn is_shared_library_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ext.eq_ignore_ascii_case("dylib")
                || ext.eq_ignore_ascii_case("so")
                || ext.eq_ignore_ascii_case("dll")
        })
}

fn looks_like_large_resource_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|filename| filename.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|filename| {
            filename.contains("onnx")
                || filename.contains("dict")
                || filename.contains("model")
                || is_shared_library_file(path)
        })
}

fn is_likely_incomplete_download_file(path: &Path) -> bool {
    std::fs::metadata(path)
        .ok()
        .filter(|metadata| metadata.len() < 1024)
        .is_some_and(|_| looks_like_large_resource_file(path))
}

fn log_remove_file(path: &Path, label: &str, success_message: &str) {
    std::fs::remove_file(path).map_or_else(
        |error| {
            crate::infrastructure::logging::warn(&format!(
                "Failed to clean up {label} {}: {}",
                path.display(),
                error
            ));
        },
        |()| {
            crate::infrastructure::logging::info(&format!(
                "{success_message}: {}",
                path.display()
            ));
        },
    );
}

#[must_use]
pub fn count_vvm_files_recursive(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };

    entries
        .flatten()
        .map(|entry| {
            let path = entry.path();
            match entry.file_type() {
                Ok(file_type) if file_type.is_file() => count_vvm_file(&path),
                Ok(file_type) if file_type.is_dir() => count_vvm_files_recursive(&path),
                _ => 0,
            }
        })
        .sum()
}

fn count_vvm_file(path: &Path) -> usize {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| {
            Path::new(name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("vvm"))
        })
        .map_or(0, |_| 1)
}

pub fn cleanup_unnecessary_files(dir: &Path) {
    let unnecessary_extensions = [".zip", ".tgz", ".tar.gz", ".tar", ".gz"];

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(file_type) if file_type.is_file() => {
                process_cleanup_file(&path, &unnecessary_extensions);
            }
            Ok(file_type) if file_type.is_dir() => {
                cleanup_unnecessary_files(&path);
                try_remove_empty_directory(&path);
            }
            _ => {}
        }
    }
}

fn process_cleanup_file(path: &Path, unnecessary_extensions: &[&str]) {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return;
    };

    if !unnecessary_extensions
        .iter()
        .any(|&ext| name.ends_with(ext))
    {
        return;
    }

    std::fs::remove_file(path).map_or_else(
        |error| crate::infrastructure::logging::warn(&format!("Failed to remove {name}: {error}")),
        |()| crate::infrastructure::logging::info(&format!("Cleaned up: {name}")),
    );
}

fn try_remove_empty_directory(path: &Path) {
    if std::fs::read_dir(path)
        .ok()
        .is_none_or(|mut entries| entries.next().is_some())
    {
        return;
    }

    if let Some(dir_name) = path.file_name().and_then(|name| name.to_str()) {
        std::fs::remove_dir(path).map_or_else(
            |error| {
                crate::infrastructure::logging::warn(&format!(
                    "Failed to remove empty directory {dir_name}: {error}"
                ));
            },
            |()| {
                crate::infrastructure::logging::info(&format!(
                    "Removed empty directory: {dir_name}"
                ));
            },
        );
    }
}
