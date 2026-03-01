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
          running_daemons, daemon_phase, pc

vars == << socket_exists, socket_responsive, socket_owner,
           running_daemons, daemon_phase, pc >>

\* ================================================================
\* Invariants
\* ================================================================

TypeOK ==
    /\ socket_exists \in BOOLEAN
    /\ socket_responsive \in BOOLEAN
    /\ socket_owner \in Daemons \cup {"nobody"}
    /\ running_daemons \subseteq Daemons
    /\ \A d \in Daemons: daemon_phase[d] \in
        {"init", "check_socket", "remove_stale", "check_pgrep",
         "bind_socket", "listening", "aborted"}

AtMostOneDaemon ==
    Cardinality({d \in Daemons: daemon_phase[d] = "listening"}) <= 1

SocketOwnerConsistency ==
    socket_exists => socket_owner \in Daemons

ListeningOwnsSocket ==
    \A d \in Daemons:
        daemon_phase[d] = "listening" =>
            /\ socket_exists
            /\ socket_responsive
            /\ socket_owner = d

ResponsiveImpliesExists ==
    socket_responsive => socket_exists

\* ================================================================
\* Initial State
\* ================================================================

Init ==
    /\ socket_exists = FALSE
    /\ socket_responsive = FALSE
    /\ socket_owner = "nobody"
    /\ running_daemons = {}
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
                                 socket_owner, running_daemons >>
            ELSE /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "remove_stale"]
                 /\ pc' = [pc EXCEPT ![d] = "RemoveStale"]
                 /\ UNCHANGED << socket_exists, socket_responsive,
                                 socket_owner, running_daemons >>
       ELSE /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "check_pgrep"]
            /\ pc' = [pc EXCEPT ![d] = "CheckPgrep"]
            /\ UNCHANGED << socket_exists, socket_responsive,
                            socket_owner, running_daemons >>

RemoveStale(d) ==
    /\ pc[d] = "RemoveStale"
    /\ IF socket_exists /\ ~socket_responsive
       THEN /\ socket_exists' = FALSE
            /\ socket_owner' = "nobody"
       ELSE /\ UNCHANGED << socket_exists, socket_owner >>
    /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "check_pgrep"]
    /\ pc' = [pc EXCEPT ![d] = "CheckPgrep"]
    /\ UNCHANGED << socket_responsive, running_daemons >>

CheckPgrep(d) ==
    /\ pc[d] = "CheckPgrep"
    /\ IF running_daemons \ {d} # {}
       THEN /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "aborted"]
            /\ pc' = [pc EXCEPT ![d] = "Done"]
       ELSE /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "bind_socket"]
            /\ pc' = [pc EXCEPT ![d] = "BindSocket"]
    /\ UNCHANGED << socket_exists, socket_responsive, socket_owner, running_daemons >>

BindSocket(d) ==
    /\ pc[d] = "BindSocket"
    /\ IF socket_exists
       THEN /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "aborted"]
            /\ UNCHANGED << socket_exists, socket_responsive,
                            socket_owner, running_daemons >>
       ELSE /\ socket_exists' = TRUE
            /\ socket_responsive' = TRUE
            /\ socket_owner' = d
            /\ running_daemons' = running_daemons \cup {d}
            /\ daemon_phase' = [daemon_phase EXCEPT ![d] = "listening"]
    /\ pc' = [pc EXCEPT ![d] = "Done"]

\* ================================================================
\* Specification
\* ================================================================

Next ==
    \E d \in Daemons:
        \/ CheckSocket(d)
        \/ RemoveStale(d)
        \/ CheckPgrep(d)
        \/ BindSocket(d)

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
    <>(\E d \in Daemons: daemon_phase[d] = "listening")

AllTerminate ==
    \A d \in Daemons: <>(daemon_phase[d] \in {"listening", "aborted"})

\* ================================================================
\* Symmetry
\* ================================================================

DaemonSymmetry == Permutations(Daemons)

=============================================================================
