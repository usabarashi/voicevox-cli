// Real-daemon synthesize model.
//
// Abstracts the client-side lifecycle that `mbt/tests/daemon_synthesize.rs`
// exercises against a real `voicevox-daemon`:
//   - connect establishes a client connection,
//   - listSpeakers reads the daemon catalog (needed to pick a valid style),
//   - synthesize requests WAV bytes for that style,
//   - disconnect drops the connection.
//
// The catalog is environment-dependent (models must be installed), so the spec
// tracks only *whether* the catalog was read and a synthesis completed; the WAV
// payload is validated in the driver.

module DaemonSynthesize {
  var connected: bool
  var catalogRead: bool
  var synthesized: bool

  action init = all {
    connected' = false,
    catalogRead' = false,
    synthesized' = false,
  }

  action connect = all {
    not(connected),
    connected' = true,
    catalogRead' = false,
    synthesized' = false,
  }

  action listSpeakers = all {
    connected,
    connected' = connected,
    catalogRead' = true,
    synthesized' = synthesized,
  }

  action synthesize = all {
    connected,
    catalogRead,
    connected' = connected,
    catalogRead' = catalogRead,
    synthesized' = true,
  }

  action disconnect = all {
    connected,
    connected' = false,
    catalogRead' = false,
    synthesized' = false,
  }

  action step = any {
    connect,
    listSpeakers,
    synthesize,
    disconnect,
  }

  val synthesizedRequiresCatalog = synthesized implies catalogRead

  // ---- Fixed scenario: always exercises a real synthesis ------------------
  //
  // Random exploration may never pick `synthesize`. This deterministic
  // scenario forces connect -> listSpeakers -> synthesize, so the MBT test
  // fails if synthesis is broken or the catalog is empty.

  action scenarioConnect = all {
    connected' = true,
    catalogRead' = false,
    synthesized' = false,
  }

  action scenarioList = all {
    connected,
    not(catalogRead),
    connected' = connected,
    catalogRead' = true,
    synthesized' = synthesized,
  }

  action scenarioSynthesize = all {
    connected,
    catalogRead,
    not(synthesized),
    connected' = connected,
    catalogRead' = catalogRead,
    synthesized' = true,
  }

  // Stutter is only enabled once a synthesis has happened, so the scenario
  // cannot terminate before reaching `synthesize`.
  action scenarioStutter = all {
    synthesized,
    connected' = connected,
    catalogRead' = catalogRead,
    synthesized' = synthesized,
  }

  action scenarioStep = any {
    scenarioList,
    scenarioSynthesize,
    scenarioStutter,
  }
}
