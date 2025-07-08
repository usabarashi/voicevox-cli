/// FD passing enabled server with proper stream handling
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::os::unix::io::AsRawFd;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

use crate::daemon::DaemonState;
use crate::ipc::{DaemonRequest, OwnedResponse};

/// Handle client with working FD passing
pub async fn handle_client_fd(
    mut stream: UnixStream,
    state: Arc<Mutex<DaemonState>>,
) -> Result<()> {
    println!("New client connected (FD-enabled handler)");

    loop {
        // Read request using framed codec
        let request = {
            let (reader, _writer) = stream.split();
            let mut framed_reader = FramedRead::new(reader, LengthDelimitedCodec::new());

            match framed_reader.next().await {
                Some(Ok(data)) => match bincode::deserialize::<DaemonRequest>(&data) {
                    Ok(req) => req,
                    Err(e) => {
                        println!("Failed to deserialize request: {}", e);
                        break;
                    }
                },
                _ => break,
            }
        };

        // Handle request
        let response = {
            let state = state.lock().await;
            state.handle_request(request).await
        };

        // Check if we need FD passing
        #[cfg(unix)]
        let needs_fd = matches!(response, OwnedResponse::SynthesizeResultFd { .. });
        #[cfg(not(unix))]
        let needs_fd = false;

        // Send response
        {
            let (_reader, writer) = stream.split();
            let mut framed_writer = FramedWrite::new(writer, LengthDelimitedCodec::new());

            match bincode::serialize(&response) {
                Ok(response_data) => {
                    if let Err(e) = framed_writer.send(response_data.into()).await {
                        println!("Failed to send response: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    println!("Failed to serialize response: {}", e);
                    break;
                }
            }
        }

        // Now handle FD passing if needed - stream is available again
        #[cfg(unix)]
        if needs_fd {
            if let Some(fd) = state.lock().await.take_pending_fd().await {
                // Small delay to ensure response is received
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                // Use the stream directly for FD passing
                let result = stream.try_io(tokio::io::Interest::WRITABLE, || {
                    use crate::daemon::fd_passing::send_fd;
                    let socket_fd = stream.as_raw_fd();
                    match send_fd(socket_fd, fd, b"audio") {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            eprintln!("FD send error: {}", e);
                            Err(std::io::Error::other(e.to_string()))
                        }
                    }
                });

                match result {
                    Ok(_) => println!("✅ Successfully sent audio FD"),
                    Err(e) => eprintln!("❌ Failed to send FD: {}", e),
                }

                // Close the FD
                unsafe {
                    libc::close(fd);
                }
            }
        }
    }

    println!("Client disconnected");
    Ok(())
}

/// Run daemon with working FD passing
pub async fn run_daemon_fd(socket_path: std::path::PathBuf, foreground: bool) -> Result<()> {
    // Remove existing socket
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    // Create listener
    let listener = UnixListener::bind(&socket_path)?;
    println!("VOICEVOX daemon started (FD-passing v2)");
    println!("Listening on: {}", socket_path.display());

    // Initialize state
    let state = Arc::new(Mutex::new(DaemonState::new().await?));

    if !foreground {
        println!("Running in background mode. Use Ctrl+C to stop gracefully.");
    }

    // Handle shutdown
    let shutdown = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for ctrl-c");
        println!("\nShutting down daemon...");
    };

    // Accept connections
    let server = async {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let state_clone = Arc::clone(&state);
                    tokio::spawn(async move {
                        if let Err(e) = handle_client_fd(stream, state_clone).await {
                            println!("Client handler error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    println!("Failed to accept connection: {}", e);
                }
            }
        }
    };

    // Run with shutdown
    tokio::select! {
        _ = server => {},
        _ = shutdown => {},
    }

    // Cleanup
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    println!("VOICEVOX daemon stopped");
    Ok(())
}
