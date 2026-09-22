// Real-daemon IPC model (Phase 3 walking skeleton).
//
// Abstracts the client-side connection/catalog-read lifecycle that
// `mbt/tests/daemon_ipc.rs` exercises against a real `voicevox-daemon` over the
// Unix socket:
//   - connect establishes a client connection to the daemon,
//   - listSpeakers (`OwnedRequest::ListSpeakers`) and listModels
//     (`OwnedRequest::ListModels`) read the daemon catalog,
//   - disconnect drops the connection.
//
// The catalog contents are environment-dependent (empty without downloaded
// models, populated with them), so this spec tracks only *whether a catalog was
// read*, not its contents. Catalog consistency is asserted in the driver.

module DaemonIpc {
  var connected: bool
  var catalogRead: bool

  action init = all {
    connected' = false,
    catalogRead' = false,
  }

  action connect = all {
    not(connected),
    connected' = true,
    catalogRead' = false,
  }

  action listSpeakers = all {
    connected,
    connected' = connected,
    catalogRead' = true,
  }

  action listModels = all {
    connected,
    connected' = connected,
    catalogRead' = true,
  }

  action disconnect = all {
    connected,
    connected' = false,
    catalogRead' = false,
  }

  action step = any {
    connect,
    listSpeakers,
    listModels,
    disconnect,
  }

  val catalogRequiresConnection = catalogRead implies connected
}
