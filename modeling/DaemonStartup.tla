------------------------------- MODULE DaemonStartup -------------------------------
(***************************************************************************)
(* Models the VOICEVOX daemon singleton startup protocol.                  *)
(*                                                                         *)
(* Verifies that concurrent startup attempts cannot result in multiple     *)
(* daemons listening on the same socket.                                   *)
(*                                                                         *)
(* Corresponding implementation:                                           *)
(*   src/daemon/process.rs     -- check_and_prevent_duplicate              *)
(*   src/daemon/server.rs      -- run_daemon, UnixListener::bind           *)
(*                                                                         *)
(* Process: DaemonStart(d) for d in Daemons                                *)
(*   CheckSocket -> [RemoveStale] -> CheckPgrep -> BindSocket              *)
(***************************************************************************)

EXTENDS Integers, FiniteSets, TLC

CONSTANTS
    Daemons,            \* Set of potential daemon instances
    MAX_RESTARTS        \* Bounded restart attempts per daemon

ASSUME Daemons # {} /\ IsFiniteSet(Daemons)
ASSUME MAX_RESTARTS \in Nat

\* ================================================================
\* Variables
\* ================================================================

VARIABLES socket_exists, socket_responsive, socket_owner,
          running_daemons, socket_path_kind, stale_remove_allowed,
          daemon_phase, restart_count, pc

vars == << socket_exists, socket_responsive, socket_owner,
           running_daemons, socket_path_kind, stale_remove_allowed,
           daemon_phase, restart_count, pc >>

\* ================================================================
\* Invariants
\* ================================================================

TypeOK ==
    /\ socket_exists \in BOOLEAN
    /\ socket_responsive \in BOOLEAN
    /\ socket_owner \in Daemons \cup {"nobody"}
    /\ running_daemons \subseteq Daemons
    /\ socket_path_kind \in {"none", "socket", "non_socket"}
    /\ stale_remove_allowed \in BOOLEAN
    /\ \A d \in Daemons: daemon_phase[d] \in
        {"init", "check_socket", "remove_stale", "check_pgrep",
         "bind_socket", "listening", "stopped", "aborted", "failed"}
    /\ \A d \in Daemons: restart_count[d] \in 0..MAX_RESTARTS
    /\ socket_exists => socket_path_kind \in {"socket", "non_socket"}
    /\ ~socket_exists => socket_path_kind = "none"

AtMostOneDaemon ==
    Cardinality({d \in Daemons: daemon_phase[d] = "listening"}) <= 1

SocketOwnerConsistency ==
    socket_exists => socket_owner \in Daemons

SocketOwnerRunningConsistency ==
    socket_responsive => socket_owner \in running_daemons

RunningDaemonBound ==
    Cardinality(running_daemons) <= 1

RunningMatchesListening ==
    running_daemons = {d \in Daemons: daemon_phase[d] = "listening"}

ListeningOwnsSocket ==
    \A d \in Daemons:
        daemon_phase[d] = "listening" =>
            /\ socket_exists
            /\ socket_responsive
            /\ socket_owner = d

ResponsiveImpliesExists ==
    socket_responsive => socket_exists

StaleRemovalFailurePath ==
    \A d \in Daemons:
        daemon_phase[d] = "failed" =>
            /\ socket_exists
            /\ ~socket_responsive
            /\ (socket_path_kind = "non_socket" \/ ~stale_remove_allowed)

\* ================================================================
\* Initial State
\* ================================================================

Init ==
    /\ socket_exists \in BOOLEAN
    /\ socket_responsive \in BOOLEAN
    /\ running_daemons = {}
    /\ socket_owner \in IF socket_exists THEN Daemons ELSE {"nobody"}
    /\ socket_path_kind \in IF socket_exists
                           THEN {"socket", "non_socket"}
                           ELSE {"none"}
    /\ stale_remove_allowed \in BOOLEAN
    /\ socket_responsive => socket_exists
    /\ socket_responsive => socket_owner \in running_daemons
    /\ daemon_phase = [d \in Daemons |-> "init"]
    /\ restart_count = [d \in Daemons |-> 0]
    /\ pc = [d \in Daemons |-> "CheckSocket"]

\* ================================================================
\* Actions
\* ================================================================

CheckSocket(d) ==
    /\ pc[d] = "CheckSocket"
    /\ IF socket_exists
       THEN IF socket_responsive
            THEN /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "aborted"]
                 /\ pc' = [pc EXCEPT ![d] = "Done"]
                 /\ UNCHANGED << socket_exists, socket_responsive,
                                 socket_owner, running_daemons,
                                 socket_path_kind, stale_remove_allowed,
                                 restart_count >>
            ELSE /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "remove_stale"]
                 /\ pc' = [pc EXCEPT ![d] = "RemoveStale"]
                 /\ UNCHANGED << socket_exists, socket_responsive,
                                 socket_owner, running_daemons,
                                 socket_path_kind, stale_remove_allowed,
                                 restart_count >>
       ELSE /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "check_pgrep"]
            /\ pc' = [pc EXCEPT ![d] = "CheckPgrep"]
            /\ UNCHANGED << socket_exists, socket_responsive,
                            socket_owner, running_daemons,
                            socket_path_kind, stale_remove_allowed,
                            restart_count >>

RemoveStale(d) ==
    /\ pc[d] = "RemoveStale"
    /\ IF socket_exists /\ ~socket_responsive
       THEN /\ IF socket_path_kind = "socket" /\ stale_remove_allowed
               THEN /\ socket_exists' = FALSE
                    /\ socket_owner' = "nobody"
                    /\ socket_path_kind' = "none"
                    /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "check_pgrep"]
                    /\ pc' = [pc EXCEPT ![d] = "CheckPgrep"]
               ELSE /\ UNCHANGED << socket_exists, socket_owner, socket_path_kind >>
                    /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "failed"]
                    /\ pc' = [pc EXCEPT ![d] = "Done"]
       ELSE /\ UNCHANGED << socket_exists, socket_owner, socket_path_kind >>
            /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "check_pgrep"]
            /\ pc' = [pc EXCEPT ![d] = "CheckPgrep"]
    /\ UNCHANGED << socket_responsive, running_daemons, stale_remove_allowed,
                    restart_count >>

CheckPgrep(d) ==
    /\ pc[d] = "CheckPgrep"
    /\ IF running_daemons \ {d} # {}
       THEN /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "aborted"]
            /\ pc' = [pc EXCEPT ![d] = "Done"]
       ELSE /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "bind_socket"]
            /\ pc' = [pc EXCEPT ![d] = "BindSocket"]
    /\ UNCHANGED << socket_exists, socket_responsive, socket_owner, running_daemons,
                    socket_path_kind, stale_remove_allowed, restart_count >>

BindSocket(d) ==
    /\ pc[d] = "BindSocket"
    /\ IF socket_exists
       THEN /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "aborted"]
            /\ UNCHANGED << socket_exists, socket_responsive,
                            socket_owner, running_daemons,
                            socket_path_kind, stale_remove_allowed,
                            restart_count >>
       ELSE /\ \/ /\ socket_exists' = TRUE
                /\ socket_responsive' = TRUE
                /\ socket_owner' = d
                /\ running_daemons' = running_daemons \cup {d}
                /\ socket_path_kind' = "socket"
                /\ UNCHANGED stale_remove_allowed
                /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "listening"]
                /\ UNCHANGED restart_count
             \/ /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "aborted"]
                /\ UNCHANGED << socket_exists, socket_responsive,
                                socket_owner, running_daemons,
                                socket_path_kind, stale_remove_allowed,
                                restart_count >>
    /\ pc' = [pc EXCEPT ![d] = "Done"]

StopRunning(d) ==
    /\ pc[d] = "Done"
    /\ daemon_phase[d] = "listening"
    /\ socket_owner = d
    /\ socket_responsive
    /\ \/ /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "stopped"]
          /\ socket_exists' = FALSE
          /\ socket_responsive' = FALSE
          /\ socket_owner' = "nobody"
          /\ running_daemons' = running_daemons \ {d}
          /\ socket_path_kind' = "none"
       \/ /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "stopped"]
          /\ socket_exists' = TRUE
          /\ socket_responsive' = FALSE
          /\ socket_owner' = d
          /\ running_daemons' = running_daemons \ {d}
          /\ socket_path_kind' = "socket"
    /\ UNCHANGED << stale_remove_allowed, restart_count, pc >>

RestartAttempt(d) ==
    /\ pc[d] = "Done"
    /\ daemon_phase[d] \in {"stopped", "aborted", "failed"}
    /\ restart_count[d] < MAX_RESTARTS
    /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "init"]
    /\ restart_count' = [restart_count EXCEPT ![d] = @ + 1]
    /\ pc' = [pc EXCEPT ![d] = "CheckSocket"]
    /\ UNCHANGED << socket_exists, socket_responsive, socket_owner,
                    running_daemons, socket_path_kind, stale_remove_allowed >>

\* ================================================================
\* Specification
\* ================================================================

Terminated ==
    \A d \in Daemons: pc[d] = "Done"

Next ==
    \/ (\E d \in Daemons:
            \/ CheckSocket(d)
            \/ RemoveStale(d)
            \/ CheckPgrep(d)
            \/ BindSocket(d)
            \/ StopRunning(d)
            \/ RestartAttempt(d))
    \/ (Terminated /\ UNCHANGED vars)

Fairness ==
    \A d \in Daemons: WF_vars(
        \/ CheckSocket(d)
        \/ RemoveStale(d)
        \/ CheckPgrep(d)
        \/ BindSocket(d)
        \/ StopRunning(d)
        \/ RestartAttempt(d)
    )

Spec == Init /\ [][Next]_vars /\ Fairness

\* ================================================================
\* Liveness
\* ================================================================

AtLeastOneStarts ==
    []((running_daemons = {} /\ ~socket_responsive
        /\ \E d \in Daemons: pc[d] # "Done")
       => <>(socket_responsive \/ \A d \in Daemons: pc[d] = "Done"))

AllTerminate ==
    <>[](\A d \in Daemons: pc[d] = "Done")

\* ================================================================
\* Symmetry
\* ================================================================

DaemonSymmetry == Permutations(Daemons)

=============================================================================
