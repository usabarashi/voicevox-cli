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
    Daemons     \* Set of potential daemon instances

ASSUME Daemons # {} /\ IsFiniteSet(Daemons)

\* ================================================================
\* Variables
\* ================================================================

VARIABLES socket_exists, socket_responsive, socket_owner,
          running_daemons, socket_path_kind, stale_remove_allowed,
          daemon_phase, pc

vars == << socket_exists, socket_responsive, socket_owner,
           running_daemons, socket_path_kind, stale_remove_allowed,
           daemon_phase, pc >>

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
         "bind_socket", "listening", "aborted", "failed"}
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
                                 socket_path_kind, stale_remove_allowed >>
            ELSE /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "remove_stale"]
                 /\ pc' = [pc EXCEPT ![d] = "RemoveStale"]
                 /\ UNCHANGED << socket_exists, socket_responsive,
                                 socket_owner, running_daemons,
                                 socket_path_kind, stale_remove_allowed >>
       ELSE /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "check_pgrep"]
            /\ pc' = [pc EXCEPT ![d] = "CheckPgrep"]
            /\ UNCHANGED << socket_exists, socket_responsive,
                            socket_owner, running_daemons,
                            socket_path_kind, stale_remove_allowed >>

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
    /\ UNCHANGED << socket_responsive, running_daemons, stale_remove_allowed >>

CheckPgrep(d) ==
    /\ pc[d] = "CheckPgrep"
    /\ IF running_daemons \ {d} # {}
       THEN /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "aborted"]
            /\ pc' = [pc EXCEPT ![d] = "Done"]
       ELSE /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "bind_socket"]
            /\ pc' = [pc EXCEPT ![d] = "BindSocket"]
    /\ UNCHANGED << socket_exists, socket_responsive, socket_owner, running_daemons,
                    socket_path_kind, stale_remove_allowed >>

BindSocket(d) ==
    /\ pc[d] = "BindSocket"
    /\ IF socket_exists
       THEN /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "aborted"]
            /\ UNCHANGED << socket_exists, socket_responsive,
                            socket_owner, running_daemons,
                            socket_path_kind, stale_remove_allowed >>
       ELSE /\ \/ /\ socket_exists' = TRUE
                /\ socket_responsive' = TRUE
                /\ socket_owner' = d
                /\ running_daemons' = running_daemons \cup {d}
                /\ socket_path_kind' = "socket"
                /\ UNCHANGED stale_remove_allowed
                /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "listening"]
             \/ /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "aborted"]
                /\ UNCHANGED << socket_exists, socket_responsive,
                                socket_owner, running_daemons,
                                socket_path_kind, stale_remove_allowed >>
    /\ pc' = [pc EXCEPT ![d] = "Done"]

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
            \/ BindSocket(d))
    \/ (Terminated /\ UNCHANGED vars)

Fairness ==
    \A d \in Daemons: WF_vars(
        \/ CheckSocket(d)
        \/ RemoveStale(d)
        \/ CheckPgrep(d)
        \/ BindSocket(d)
    )

Spec == Init /\ [][Next]_vars /\ Fairness

\* ================================================================
\* Liveness
\* ================================================================

AtLeastOneStarts ==
    (running_daemons = {} /\ ~socket_responsive
     /\ Cardinality({d \in Daemons: pc[d] = "CheckSocket"}) >= 1)
        => <>(socket_responsive
              \/ \A d \in Daemons: daemon_phase[d] \in {"aborted", "failed"})

AllTerminate ==
    \A d \in Daemons: <>(daemon_phase[d] \in {"listening", "aborted", "failed"})

\* ================================================================
\* Symmetry
\* ================================================================

DaemonSymmetry == Permutations(Daemons)

=============================================================================
