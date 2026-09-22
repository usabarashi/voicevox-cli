// Daemon IPC server connection/request lifecycle model.
//
// Mirrors `src/infrastructure/daemon/server.rs`:
//   - `accept_loop` spawns one task per accepted client,
//   - `handle_client_with_limit` acquires a shared permit per request before
//     calling `handle_request` and releases it afterwards (`MAX_CONCURRENT_CLIENTS`
//     = 32 concurrent request handlings across all clients),
//   - a client whose read times out (`CLIENT_IDLE_TIMEOUT` = 30 s) or whose
//     frame fails to decode is closed,
//   - requests from one client are handled sequentially.
//
// The synthesis worker mutex itself is modelled separately in
// `DaemonSerialization.qnt`.

module DaemonServer {
  pure val MAX_IN_FLIGHT = 32
  pure val CLIENT_IDS = Set(0, 1, 2)

  type ClientPhase = Absent | Connected | Handling | Closed

  var client: int -> ClientPhase
  var inFlight: int

  action init = all {
    client' = Map(0 -> Absent, 1 -> Absent, 2 -> Absent),
    inFlight' = 0,
  }

  action accept = all {
    nondet id = oneOf(CLIENT_IDS)
    all {
      client.get(id) == Absent,
      client' = client.set(id, Connected),
      inFlight' = inFlight,
    }
  }

  // Admit one request for a connected client (acquires a permit).
  action startHandling = all {
    nondet id = oneOf(CLIENT_IDS)
    all {
      client.get(id) == Connected,
      inFlight < MAX_IN_FLIGHT,
      client' = client.set(id, Handling),
      inFlight' = inFlight + 1,
    }
  }

  action finish0 = all {
    client.get(0) == Handling,
    client' = client.set(0, Connected),
    inFlight' = inFlight - 1,
  }

  action finish1 = all {
    client.get(1) == Handling,
    client' = client.set(1, Connected),
    inFlight' = inFlight - 1,
  }

  action finish2 = all {
    client.get(2) == Handling,
    client' = client.set(2, Connected),
    inFlight' = inFlight - 1,
  }

  // No frame within the idle timeout: the connection is closed.
  action idleTimeout = all {
    nondet id = oneOf(CLIENT_IDS)
    all {
      client.get(id) == Connected,
      client' = client.set(id, Closed),
      inFlight' = inFlight,
    }
  }

  // An undecodable frame closes the connection.
  action decodeFail = all {
    nondet id = oneOf(CLIENT_IDS)
    all {
      client.get(id) == Connected,
      client' = client.set(id, Closed),
      inFlight' = inFlight,
    }
  }

  action stutter = all {
    client' = client,
    inFlight' = inFlight,
  }

  action step = any {
    accept,
    startHandling,
    finish0,
    finish1,
    finish2,
    idleTimeout,
    decodeFail,
    stutter,
  }

  // ---- Safety -------------------------------------------------------------

  val typeOK = inFlight >= 0 and inFlight <= MAX_IN_FLIGHT

  val inFlightMatchesHandling =
    inFlight ==
      (if (client.get(0) == Handling) 1 else 0) +
      (if (client.get(1) == Handling) 1 else 0) +
      (if (client.get(2) == Handling) 1 else 0)

  // ---- Liveness -----------------------------------------------------------

  val vars = (client, inFlight)

  // A client's request always finishes (no handler is stuck). Per-client weak
  // fairness is required; a single disjunctive fairness set would not force the
  // specific client's `finish` action.
  temporal handling0Terminates =
    weakFair(finish0, vars) implies
      always(client.get(0) == Handling implies eventually(client.get(0) != Handling))

  temporal handling1Terminates =
    weakFair(finish1, vars) implies
      always(client.get(1) == Handling implies eventually(client.get(1) != Handling))

  temporal handling2Terminates =
    weakFair(finish2, vars) implies
      always(client.get(2) == Handling implies eventually(client.get(2) != Handling))
}
