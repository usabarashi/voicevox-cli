//! End-to-end checks on the `voicevox-mcp-server` stdio contract.
//!
//! These run the real binary as a subprocess, so they cover wiring that unit
//! tests cannot: the startup opt-in that keeps informational logs off stdout, and
//! the shape of the frames the server emits for the stateless `2026-07-28`
//! revision.
//!
//! Each run has a definite end: responses are awaited by id with a bounded
//! timeout, then stdin is closed and stdout is drained to EOF before the child is
//! reaped. Only after that boundary are the purity assertions applied, so a frame
//! or stray line written late in shutdown cannot escape them.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const META: &str = r#""_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}"#;

/// Upper bound on waiting for an expected response.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);

/// Upper bound on shutdown once stdin is closed.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_voicevox-mcp-server")
}

fn request(id: u32, method: &str, params: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{{{params}}}}}"#)
}

/// A running server, its frame stream, and every frame seen so far.
struct ServerRun {
    child: Child,
    stdin: Option<ChildStdin>,
    frames: mpsc::Receiver<String>,
    collected: Vec<String>,
    reader: Option<std::thread::JoinHandle<()>>,
}

impl ServerRun {
    /// Starts the server with its socket under `socket_root`.
    fn start(requests: &[String], socket_root: &std::path::Path) -> Self {
        Self::start_with(requests, socket_root, Stdio::null())
    }

    /// Starts the server with `stderr` handled by the caller.
    fn start_with(requests: &[String], socket_root: &std::path::Path, stderr: Stdio) -> Self {
        let mut child = Command::new(binary())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr)
            .env("VOICEVOX_SOCKET_PATH", socket_root.join("absent.sock"))
            .spawn()
            .expect("spawn voicevox-mcp-server");

        {
            let stdin = child.stdin.as_mut().expect("stdin is piped");
            for request in requests {
                writeln!(stdin, "{request}").expect("write request");
            }
            stdin.flush().expect("flush requests");
        }

        let stdout = child.stdout.take().expect("stdout is piped");
        let (tx, frames) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) if !line.trim().is_empty() => {
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
        });

        let stdin = child.stdin.take();
        Self {
            child,
            stdin,
            frames,
            collected: Vec::new(),
            reader: Some(reader),
        }
    }

    /// Moves any frame the reader has already delivered into `collected`.
    fn drain_pending(&mut self) {
        while let Ok(line) = self.frames.try_recv() {
            self.collected.push(line);
        }
    }

    /// Waits until `id` has been answered, returning that response.
    fn await_response(&mut self, id: u32) -> Value {
        let deadline = Instant::now() + RESPONSE_TIMEOUT;

        loop {
            self.drain_pending();
            if let Some(found) = self
                .collected
                .iter()
                .map(|line| parse_frame(line))
                .find(|value| value["id"] == json!(id))
            {
                return found;
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "timed out waiting for response id {id}; frames so far: {:?}",
                self.collected
            );
            match self.frames.recv_timeout(remaining) {
                Ok(line) => self.collected.push(line),
                Err(_) => panic!(
                    "server produced no response for id {id}; frames so far: {:?}",
                    self.collected
                ),
            }
        }
    }

    /// Closes stdin (the stdio shutdown signal), drains stdout to EOF, and reaps
    /// the child.
    ///
    /// After this returns, `collected` holds everything the server ever wrote to
    /// stdout, so assertions over it are complete rather than merely "so far".
    fn shutdown(&mut self) {
        drop(self.stdin.take());

        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match self.frames.recv_timeout(remaining) {
                Ok(line) => self.collected.push(line),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => break,
            }
        }

        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        // `wait` reaps the child; a kill here would defeat the drain above.
        let _ = self.child.wait();
    }

    /// Asserts that every frame the server ever wrote was a JSON-RPC 2.0 message.
    fn assert_only_frames(&self) {
        for line in &self.collected {
            let parsed = parse_frame(line);
            assert_eq!(parsed["jsonrpc"], json!("2.0"), "not a frame: {line:?}");
            assert_envelope(&parsed);
        }
    }
}

impl Drop for ServerRun {
    fn drop(&mut self) {
        // Only reached without a prior shutdown (e.g. an assertion panicked).
        drop(self.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn parse_frame(line: &str) -> Value {
    serde_json::from_str(line)
        .unwrap_or_else(|error| panic!("stdout line is not JSON ({error}): {line:?}"))
}

/// Checks the shape of one response against the schema's two envelope types.
///
/// A JSON-RPC message is either a result response or an error response, never a
/// mix: the result envelope carries `resultType`/`_meta`, while the error
/// envelope carries only `error` and must not grow result metadata.
fn assert_envelope(frame: &Value) {
    let object = frame.as_object().expect("frame is a JSON object");
    let has_result = object.contains_key("result");
    let has_error = object.contains_key("error");
    assert!(
        has_result ^ has_error,
        "a response is exactly one of result/error: {frame}"
    );

    if has_result {
        for key in object.keys() {
            assert!(
                matches!(key.as_str(), "jsonrpc" | "id" | "result"),
                "unexpected member in a result response: {key} in {frame}"
            );
        }
        assert!(
            frame["result"]["resultType"].is_string(),
            "every result carries resultType: {frame}"
        );
        assert!(
            frame["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"].is_string(),
            "every result carries serverInfo: {frame}"
        );
        return;
    }

    for key in object.keys() {
        assert!(
            matches!(key.as_str(), "jsonrpc" | "id" | "error"),
            "unexpected member in an error response: {key} in {frame}"
        );
    }
    let error = frame["error"].as_object().expect("error is an object");
    for key in error.keys() {
        assert!(
            matches!(key.as_str(), "code" | "message" | "data"),
            "unexpected member in a JSON-RPC error: {key} in {frame}"
        );
    }
    assert!(frame["error"]["code"].is_i64(), "error code is an integer");
    assert!(
        frame["error"]["message"].is_string(),
        "error message is a string"
    );
}

/// One complete server session with an unusable socket path.
///
/// The response for `id` is returned together with the full set of frames the
/// server produced, collected after it exited.
fn run(requests: &[String], id: u32) -> (Value, Vec<String>) {
    let dir = tempfile::tempdir().expect("temp dir");
    // The parent of the socket path is a regular file, so binding fails with
    // ENOTDIR immediately. This is deliberate and does not depend on the host:
    // the MCP server starts the daemon before serving any request, so a socket
    // path that *could* be bound would make every test spawn a real daemon and
    // wait out its readiness budget (about 11s each, and worse in parallel).
    let blocker = dir.path().join("not-a-directory");
    std::fs::write(&blocker, b"").expect("create blocking file");

    let mut server = ServerRun::start(requests, &blocker);
    let response = server.await_response(id);
    server.shutdown();

    server.assert_only_frames();
    (response, server.collected.clone())
}

#[test]
fn server_discover_reports_identity_capabilities_and_cache_hints() {
    let (response, _frames) = run(&[request(1, "server/discover", META)], 1);

    let result = &response["result"];
    assert_eq!(result["resultType"], json!("complete"));
    assert_eq!(result["supportedVersions"], json!(["2026-07-28"]));
    assert!(result["ttlMs"].is_u64(), "ttlMs must be present: {result}");
    assert_eq!(result["cacheScope"], json!("public"));
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        json!("voicevox-mcp"),
        "serverInfo must identify the server: {result}"
    );
}

#[test]
fn tools_list_carries_result_type_and_cache_hints() {
    let (response, _frames) = run(&[request(1, "tools/list", META)], 1);

    let result = &response["result"];
    assert_eq!(result["resultType"], json!("complete"));
    assert!(result["ttlMs"].is_u64());
    assert_eq!(result["cacheScope"], json!("public"));

    let names: Vec<&str> = result["tools"]
        .as_array()
        .expect("tools is an array")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(names.contains(&"text_to_speech"), "tools: {names:?}");
    assert!(names.contains(&"list_voice_styles"), "tools: {names:?}");

    for tool in result["tools"].as_array().expect("array") {
        assert_eq!(tool["inputSchema"]["type"], json!("object"));
    }
}

#[test]
fn modern_requests_need_no_handshake() {
    // No `initialize` first: the revision is stateless, so a bare `tools/list`
    // must be served.
    let (response, _frames) = run(&[request(7, "tools/list", META)], 7);

    assert!(
        response.get("result").is_some(),
        "a modern request must be served without initialization: {response}"
    );
}

#[test]
fn initialize_is_rejected_and_names_supported_versions() {
    let (response, _frames) = run(&[request(2, "initialize", "")], 2);

    let error = &response["error"];
    assert_eq!(error["code"], json!(-32601));
    assert_eq!(error["data"]["supported"], json!(["2026-07-28"]));
}

#[test]
fn missing_meta_is_invalid_params_but_a_wrong_version_is_retryable() {
    // Malformed: nothing for the client to retry.
    let (malformed, _frames) = run(&[request(3, "tools/list", "")], 3);
    assert_eq!(malformed["error"]["code"], json!(-32602));

    // Well-formed but unsupported: retryable against the listed versions.
    let (unsupported, _frames) = run(
        &[request(
            4,
            "tools/list",
            r#""_meta":{"io.modelcontextprotocol/protocolVersion":"2025-06-18","io.modelcontextprotocol/clientCapabilities":{}}"#,
        )],
        4,
    );
    let error = &unsupported["error"];
    assert_eq!(error["code"], json!(-32022));
    assert_eq!(error["data"]["requested"], json!("2025-06-18"));
    assert_eq!(error["data"]["supported"], json!(["2026-07-28"]));
}

#[test]
fn unknown_tool_is_a_protocol_error_not_a_tool_result() {
    // MCP tools#error-handling: an unknown tool is a request-structure problem,
    // so it must be a JSON-RPC error. Reported as `isError` it would invite the
    // model to retry with different arguments, which cannot help.
    let params = format!(r#"{META},"name":"no_such_tool","arguments":{{}}"#);
    let (response, _frames) = run(&[request(5, "tools/call", &params)], 5);

    assert!(
        response.get("result").is_none(),
        "must not be reported as a tool result: {response}"
    );
    let error = &response["error"];
    assert_eq!(error["code"], json!(-32602));
    assert!(
        error["message"]
            .as_str()
            .unwrap_or_default()
            .contains("no_such_tool"),
        "error must name the tool: {error}"
    );
}

#[test]
fn known_tool_answers_with_a_tool_result() {
    // The async handler path: a known tool whose arguments are invalid is an
    // *execution* error, so it must arrive as a tool result with `isError`
    // rather than a protocol error.
    //
    // `style_id` is rejected before any daemon contact, so this asserts on the
    // handler's error reporting without depending on a daemon being reachable.
    let params = format!(
        r#"{META},"name":"text_to_speech","arguments":{{"text":"テスト","style_id":4294967295}}"#
    );
    let (response, _frames) = run(&[request(9, "tools/call", &params)], 9);

    let result = &response["result"];
    assert_eq!(result["resultType"], json!("complete"));
    assert_eq!(result["isError"], json!(true));
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        json!("voicevox-mcp"),
        "tool results carry serverInfo too: {result}"
    );
}

#[test]
fn stdout_stays_pure_when_a_tool_call_is_rejected() {
    // Rejection paths format strings too; they must stay inside the JSON frame.
    let params = format!(r#"{META},"name":"no_such_tool","arguments":{{}}"#);
    let (_response, frames) = run(&[request(8, "tools/call", &params)], 8);

    assert_eq!(
        frames.len(),
        1,
        "exactly one frame expected, got: {frames:?}"
    );
}

#[test]
fn startup_logging_never_reaches_stdout() {
    // Startup logs through `AppOutput`, and the socket points at an absent path
    // in a private directory, so the failure path logs too. Everything the server
    // wrote to stdout over the whole session must still be frames.
    let (_response, frames) = run(&[request(6, "server/discover", META)], 6);

    assert_eq!(
        frames.len(),
        1,
        "exactly one frame expected, got: {frames:?}"
    );
}

#[test]
fn startup_info_logging_goes_to_stderr_not_stdout() {
    // Regression guard for the stdout-purity fix on the *daemon startup* path.
    //
    // `run` points the socket at an unbindable path, which fails before the
    // daemon start sequence logs anything, so it cannot cover this. Here the
    // server runs from a private directory holding a stub `voicevox-daemon` that
    // exits with `PERMISSION_DENIED`, so `start_daemon_automatically` runs its
    // full `logging::info` sequence before failing fast. A real daemon is never
    // involved and nothing outside the temp dir is touched.
    let dir = tempfile::tempdir().expect("temp dir");
    let server_dir = dir.path().join("bin");
    std::fs::create_dir(&server_dir).expect("create bin dir");

    // Copy the server next to a stub daemon: `find_daemon_binary` looks for
    // `voicevox-daemon` beside the running executable and returns it as soon as
    // the file exists, so this stub is what gets executed and there is no
    // fallback to a daemon elsewhere on the system.
    let server_path = server_dir.join("voicevox-mcp-server");
    std::fs::copy(binary(), &server_path).expect("copy server binary");
    let stub = server_dir.join("voicevox-daemon");
    // The marker makes the stub's execution an assertion rather than an
    // assumption: without it the test could pass via some other daemon path, so
    // the stub itself has to create it before reporting `PERMISSION_DENIED`.
    let marker = server_dir.join("stub-ran");
    // The stub writes its marker through shell redirection into its own
    // directory, so it needs no external tool and no path is interpolated into
    // shell source (`${0%/*}` is POSIX parameter expansion, not `dirname`).
    std::fs::write(&stub, "#!/bin/sh\n: > \"${0%/*}/stub-ran\"\nexit 3\n")
        .expect("write stub daemon");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
            .expect("chmod stub daemon");
    }

    let socket_dir = tempfile::tempdir().expect("socket temp dir");
    let mut child = Command::new(&server_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env(
            "VOICEVOX_SOCKET_PATH",
            socket_dir.path().join("absent.sock"),
        )
        .spawn()
        .expect("spawn server copy");

    {
        let stdin = child.stdin.as_mut().expect("stdin is piped");
        let meta = r#""_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}"#;
        let params = format!(r#"{meta},"name":"list_voice_styles","arguments":{{}}"#);
        writeln!(stdin, "{}", request(1, "tools/call", &params)).expect("write request");
        stdin.flush().expect("flush");
    }

    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");

    // Read stdout and stderr concurrently so a full pipe on one cannot stall the
    // other.
    let stdout_reader = std::thread::spawn(move || {
        BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
    });
    let stderr_reader = std::thread::spawn(move || {
        BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
            .collect::<Vec<_>>()
    });

    let status = child.wait().expect("server exits");
    let stdout_lines = stdout_reader.join().expect("join stdout reader");
    let stderr_lines = stderr_reader.join().expect("join stderr reader");

    assert!(status.success(), "server exited with {status}");

    // The startup sequence really ran: these are `logging::info` calls. Without
    // this assertion the test would pass vacuously whenever startup fails before
    // logging anything.
    let stderr_text = stderr_lines.join("\n");
    assert!(
        stderr_text.contains("Starting VOICEVOX daemon"),
        "startup info logging did not run; stderr:\n{stderr_text}"
    );
    assert!(
        stderr_text.contains("Starting daemon process"),
        "expected the full startup sequence; stderr:\n{stderr_text}"
    );

    // The stub is the daemon that ran, so nothing outside these temp dirs was
    // involved. The marker turns that into an assertion.
    assert!(
        marker.is_file(),
        "the stub daemon did not run; the startup path used something else"
    );

    // ...and none of it leaked onto stdout, which stays JSON-RPC only.
    //
    // stdin is closed immediately here, so the tool call may never be answered;
    // that is fine, because this test is about startup logging rather than the
    // response. What matters is that every line stdout did receive is a frame.
    for line in &stdout_lines {
        let parsed = parse_frame(line);
        assert_eq!(
            parsed["jsonrpc"],
            json!("2.0"),
            "startup log leaked onto stdout: {line:?}"
        );
    }
}
