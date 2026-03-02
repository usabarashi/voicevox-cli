---------------------------------- MODULE Model -----------------------------------
(***************************************************************************)
(* Shared state vocabularies for minimal VOICEVOX formal models.           *)
(***************************************************************************)

EXTENDS Naturals

DaemonStates == {"DaemonDown", "Starting", "Ready", "Recovering"}
SocketStates == {"SocketAbsent", "SocketReady"}
ClientStates == {"Idle", "Connecting", "Connected", "Failed"}
RequestStates == {"Idle", "Busy"}

ValidRetry(n, max) == n \in 0..max

=============================================================================
