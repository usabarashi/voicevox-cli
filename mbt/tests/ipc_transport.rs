//! Model-based test for the IPC transport contract.
//!
//! A fake Unix-socket server (no daemon) injects a response class, and the
//! **production** `DaemonClient::synthesize` is driven against it. Quint Connect
//! compares the observed request state plus the injected response class against
//! the fixed scenarios in `modeling/quint/IPC.qnt`.
//!
//! Observation boundary: the injected response class is environment input (the
//! fake server), while `reqState` is observed from whether production returned
//! `Ok`/`Err`. A production change that accepted a fault as a valid response
//! would make `reqState` diverge from the spec.

use anyhow::Result as AnyResult;
use quint_connect::*;
use serde::Deserialize;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use voicevox_cli::infrastructure::daemon::client::DaemonClient;
use voicevox_cli::infrastructure::ipc::{DaemonErrorCode, OwnedResponse, OwnedSynthesizeOptions};

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum ConnState {
    Disconnected,
    Connected,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum ReqState {
    Idle,
    InFlight,
    Done,
    Failed,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum RespState {
    NoResponse,
    Valid,
    Corrupt,
    Mismatched,
    Timeout,
    Absent,
    TooLarge,
    ProtocolError,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum IpcError {
    NoError,
    EncodeFailed,
    WriteFailed,
    CorruptFrame,
    ResponseMismatch,
    ResponseTimeout,
    NoResponseReceived,
    FrameTooLarge,
    ErrorResponse,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct IpcState {
    #[serde(rename = "connState")]
    conn_state: ConnState,
    #[serde(rename = "reqState")]
    req_state: ReqState,
    #[serde(rename = "respState")]
    resp_state: RespState,
    #[serde(rename = "ipcError")]
    ipc_error: IpcError,
}

/// What the fake server does in response to one request.
#[derive(Clone, Copy)]
enum Fault {
    Valid,
    Corrupt,
    Mismatch,
    NoResponse,
    ProtocolError,
}

impl Fault {
    fn payload(self) -> Option<Vec<u8>> {
        match self {
            Self::Valid => Some(
                postcard::to_allocvec(&OwnedResponse::SynthesizeResult {
                    wav_data: vec![1, 2, 3],
                })
                .expect("encode"),
            ),
            Self::ProtocolError => Some(
                postcard::to_allocvec(&OwnedResponse::Error {
                    code: DaemonErrorCode::SynthesisFailed,
                    message: "boom".to_string(),
                })
                .expect("encode"),
            ),
            Self::Mismatch => Some(
                postcard::to_allocvec(&OwnedResponse::ModelsList { models: vec![] })
                    .expect("encode"),
            ),
            // A zero-length frame cannot decode as an `OwnedResponse`.
            Self::Corrupt => Some(Vec::new()),
            Self::NoResponse => None,
        }
    }

    fn expected(self) -> (RespState, IpcError) {
        match self {
            Self::Valid => (RespState::Valid, IpcError::NoError),
            Self::Corrupt => (RespState::Corrupt, IpcError::CorruptFrame),
            Self::Mismatch => (RespState::Mismatched, IpcError::ResponseMismatch),
            Self::NoResponse => (RespState::Absent, IpcError::NoResponseReceived),
            Self::ProtocolError => (RespState::ProtocolError, IpcError::ErrorResponse),
        }
    }
}

/// Reads one length-delimited request, then writes the chosen response frame
/// (or closes for `None`).
async fn serve_once(listener: UnixListener, payload: Option<Vec<u8>>) {
    let Ok((mut stream, _)) = listener.accept().await else {
        return;
    };
    let mut len_buf = [0u8; 4];
    if stream.read_exact(&mut len_buf).await.is_ok() {
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut request = vec![0u8; len];
        let _ = stream.read_exact(&mut request).await;
    }
    if let Some(bytes) = payload {
        let len = u32::try_from(bytes.len()).expect("small payload");
        let _ = stream.write_all(&len.to_be_bytes()).await;
        let _ = stream.write_all(&bytes).await;
        let _ = stream.flush().await;
    }
}

struct IpcDriver {
    runtime: tokio::runtime::Runtime,
    conn_state: ConnState,
    req_state: ReqState,
    resp_state: RespState,
    ipc_error: IpcError,
}

impl Default for IpcDriver {
    fn default() -> Self {
        Self {
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
            conn_state: ConnState::Disconnected,
            req_state: ReqState::Idle,
            resp_state: RespState::NoResponse,
            ipc_error: IpcError::NoError,
        }
    }
}

impl IpcDriver {
    fn reset(&mut self) {
        self.conn_state = ConnState::Disconnected;
        self.req_state = ReqState::Idle;
        self.resp_state = RespState::NoResponse;
        self.ipc_error = IpcError::NoError;
    }

    fn run(&mut self, fault: Fault) {
        let dir = secure_tempdir();
        let socket = dir.path().join("fake-daemon.sock");
        let listener = {
            // `UnixListener::bind` registers with the runtime reactor.
            let _guard = self.runtime.enter();
            UnixListener::bind(&socket).expect("bind fake daemon socket")
        };
        // `bind` applies the process umask, so with umask 000 the socket could
        // be world-writable and `DaemonClient::new_at`'s validation would
        // reject it. Pin the mode so the test is umask-independent.
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))
                .expect("chmod 0600 on the fake socket");
        }
        let server = self.runtime.spawn(serve_once(listener, fault.payload()));

        let mut client = self
            .runtime
            .block_on(DaemonClient::new_at(&socket))
            .expect("connect to fake daemon");
        let result = self.runtime.block_on(client.synthesize(
            "テスト",
            3,
            OwnedSynthesizeOptions { rate: 1.0 },
        ));
        let _ = self.runtime.block_on(server);

        self.conn_state = ConnState::Connected;
        self.req_state = if result.is_ok() {
            ReqState::Done
        } else {
            ReqState::Failed
        };
        let (resp, err) = fault.expected();
        self.resp_state = resp;
        self.ipc_error = err;
    }
}

fn secure_tempdir() -> TempDir {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
        .expect("chmod 0700");
    dir
}

impl State<IpcDriver> for IpcState {
    fn from_driver(driver: &IpcDriver) -> AnyResult<Self> {
        Ok(Self {
            conn_state: driver.conn_state.clone(),
            req_state: driver.req_state.clone(),
            resp_state: driver.resp_state.clone(),
            ipc_error: driver.ipc_error.clone(),
        })
    }
}

impl Driver for IpcDriver {
    type State = IpcState;

    // clippy 1.98 flags quint-connect's `switch!` expansion as `no_effect`.
    #[allow(clippy::no_effect)]
    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.reset(),
            step => self.reset(),
            hold => (),
            scenarioValid => self.run(Fault::Valid),
            scenarioCorrupt => self.run(Fault::Corrupt),
            scenarioMismatch => self.run(Fault::Mismatch),
            scenarioNoResponse => self.run(Fault::NoResponse),
            scenarioProtocolError => self.run(Fault::ProtocolError),
            _ => (),
        })
    }
}

/// A valid response yields `Done`.
#[quint_run(
    spec = "../modeling/quint/IPC.qnt",
    init = "scenarioValid",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn ipc_valid_response() -> impl Driver {
    IpcDriver::default()
}

/// An undecodable frame fails.
#[quint_run(
    spec = "../modeling/quint/IPC.qnt",
    init = "scenarioCorrupt",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn ipc_corrupt_frame() -> impl Driver {
    IpcDriver::default()
}

/// A valid frame of the wrong variant fails.
#[quint_run(
    spec = "../modeling/quint/IPC.qnt",
    init = "scenarioMismatch",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn ipc_response_mismatch() -> impl Driver {
    IpcDriver::default()
}

/// A closed stream without a response fails.
#[quint_run(
    spec = "../modeling/quint/IPC.qnt",
    init = "scenarioNoResponse",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn ipc_no_response() -> impl Driver {
    IpcDriver::default()
}

/// A protocol `Error` response fails.
#[quint_run(
    spec = "../modeling/quint/IPC.qnt",
    init = "scenarioProtocolError",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn ipc_protocol_error() -> impl Driver {
    IpcDriver::default()
}
