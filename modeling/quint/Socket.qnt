// Daemon IPC socket lifecycle model.
//
// Production binds the Unix socket with a single `UnixListener::bind` call
// (`src/infrastructure/daemon/server.rs`): there is **no bind retry**.
// Permission denial fails the daemon (exit code 3). The client-side connect
// retry is modelled separately (`MCPServer.qnt` / `transport::retry_with_final`).
//
// `socketDrop` models the socket file being removed on daemon shutdown.

module Socket {
  type SocketState = SocketAbsent | Binding | SocketReady | PermissionDenied

  var state: SocketState

  action init = all {
    state' = SocketAbsent,
  }

  action startBind = all {
    state == SocketAbsent,
    state' = Binding,
  }

  action bindOk = all {
    state == Binding,
    state' = SocketReady,
  }

  action bindPermissionDenied = all {
    state == Binding,
    state' = PermissionDenied,
  }

  action socketDrop = all {
    state == SocketReady,
    state' = SocketAbsent,
  }

  action stutter = all {
    state' = state,
  }

  action step = any {
    startBind,
    bindOk,
    bindPermissionDenied,
    socketDrop,
    stutter,
  }

  action advance = any {
    startBind,
    bindOk,
    bindPermissionDenied,
    socketDrop,
  }

  val vars = (state, state)

  // A bind attempt always leaves `Binding`.
  temporal bindingTerminates =
    weakFair(advance, vars) implies
      always(state == Binding implies eventually(state != Binding))

  // A permission denial is terminal for that daemon run.
  temporal permissionDeniedIsTerminal =
    always(state == PermissionDenied implies always(state == PermissionDenied))
}
