---- MODULE DaemonStartup_TTrace_1772410611 ----
EXTENDS DaemonStartup_TEConstants, Sequences, TLCExt, Toolbox, DaemonStartup, Naturals, TLC

_expression ==
    LET DaemonStartup_TEExpression == INSTANCE DaemonStartup_TEExpression
    IN DaemonStartup_TEExpression!expression
----

_trace ==
    LET DaemonStartup_TETrace == INSTANCE DaemonStartup_TETrace
    IN DaemonStartup_TETrace!trace
----

_inv ==
    ~(
        TLCGet("level") = Len(_TETrace)
        /\
        daemon_phase = ((d1 :> "listening" @@ d2 :> "aborted" @@ d3 :> "aborted"))
        /\
        pc = ((d1 :> "Done" @@ d2 :> "Done" @@ d3 :> "Done"))
        /\
        socket_owner = (d1)
        /\
        socket_exists = (TRUE)
        /\
        socket_responsive = (TRUE)
        /\
        running_daemons = ({d1})
    )
----

_init ==
    /\ socket_owner = _TETrace[1].socket_owner
    /\ running_daemons = _TETrace[1].running_daemons
    /\ pc = _TETrace[1].pc
    /\ socket_responsive = _TETrace[1].socket_responsive
    /\ socket_exists = _TETrace[1].socket_exists
    /\ daemon_phase = _TETrace[1].daemon_phase
----

_next ==
    /\ \E i,j \in DOMAIN _TETrace:
        /\ \/ /\ j = i + 1
              /\ i = TLCGet("level")
        /\ socket_owner  = _TETrace[i].socket_owner
        /\ socket_owner' = _TETrace[j].socket_owner
        /\ running_daemons  = _TETrace[i].running_daemons
        /\ running_daemons' = _TETrace[j].running_daemons
        /\ pc  = _TETrace[i].pc
        /\ pc' = _TETrace[j].pc
        /\ socket_responsive  = _TETrace[i].socket_responsive
        /\ socket_responsive' = _TETrace[j].socket_responsive
        /\ socket_exists  = _TETrace[i].socket_exists
        /\ socket_exists' = _TETrace[j].socket_exists
        /\ daemon_phase  = _TETrace[i].daemon_phase
        /\ daemon_phase' = _TETrace[j].daemon_phase

\* Uncomment the ASSUME below to write the states of the error trace
\* to the given file in Json format. Note that you can pass any tuple
\* to `JsonSerialize`. For example, a sub-sequence of _TETrace.
    \* ASSUME
    \*     LET J == INSTANCE Json
    \*         IN J!JsonSerialize("DaemonStartup_TTrace_1772410611.json", _TETrace)

=============================================================================

 Note that you can extract this module `DaemonStartup_TEExpression`
  to a dedicated file to reuse `expression` (the module in the 
  dedicated `DaemonStartup_TEExpression.tla` file takes precedence 
  over the module `DaemonStartup_TEExpression` below).

---- MODULE DaemonStartup_TEExpression ----
EXTENDS DaemonStartup_TEConstants, Sequences, TLCExt, Toolbox, DaemonStartup, Naturals, TLC

expression == 
    [
        \* To hide variables of the `DaemonStartup` spec from the error trace,
        \* remove the variables below.  The trace will be written in the order
        \* of the fields of this record.
        socket_owner |-> socket_owner
        ,running_daemons |-> running_daemons
        ,pc |-> pc
        ,socket_responsive |-> socket_responsive
        ,socket_exists |-> socket_exists
        ,daemon_phase |-> daemon_phase
        
        \* Put additional constant-, state-, and action-level expressions here:
        \* ,_stateNumber |-> _TEPosition
        \* ,_socket_ownerUnchanged |-> socket_owner = socket_owner'
        
        \* Format the `socket_owner` variable as Json value.
        \* ,_socket_ownerJson |->
        \*     LET J == INSTANCE Json
        \*     IN J!ToJson(socket_owner)
        
        \* Lastly, you may build expressions over arbitrary sets of states by
        \* leveraging the _TETrace operator.  For example, this is how to
        \* count the number of times a spec variable changed up to the current
        \* state in the trace.
        \* ,_socket_ownerModCount |->
        \*     LET F[s \in DOMAIN _TETrace] ==
        \*         IF s = 1 THEN 0
        \*         ELSE IF _TETrace[s].socket_owner # _TETrace[s-1].socket_owner
        \*             THEN 1 + F[s-1] ELSE F[s-1]
        \*     IN F[_TEPosition - 1]
    ]

=============================================================================



Parsing and semantic processing can take forever if the trace below is long.
 In this case, it is advised to uncomment the module below to deserialize the
 trace from a generated binary file.

\*
\*---- MODULE DaemonStartup_TETrace ----
\*EXTENDS DaemonStartup_TEConstants, IOUtils, DaemonStartup, TLC
\*
\*trace == IODeserialize("DaemonStartup_TTrace_1772410611.bin", TRUE)
\*
\*=============================================================================
\*

---- MODULE DaemonStartup_TETrace ----
EXTENDS DaemonStartup_TEConstants, DaemonStartup, TLC

trace == 
    <<
    ([daemon_phase |-> (d1 :> "init" @@ d2 :> "init" @@ d3 :> "init"),pc |-> (d1 :> "CheckSocket" @@ d2 :> "CheckSocket" @@ d3 :> "CheckSocket"),socket_owner |-> "nobody",socket_exists |-> FALSE,socket_responsive |-> FALSE,running_daemons |-> {}]),
    ([daemon_phase |-> (d1 :> "check_pgrep" @@ d2 :> "init" @@ d3 :> "init"),pc |-> (d1 :> "CheckPgrep" @@ d2 :> "CheckSocket" @@ d3 :> "CheckSocket"),socket_owner |-> "nobody",socket_exists |-> FALSE,socket_responsive |-> FALSE,running_daemons |-> {}]),
    ([daemon_phase |-> (d1 :> "bind_socket" @@ d2 :> "init" @@ d3 :> "init"),pc |-> (d1 :> "BindSocket" @@ d2 :> "CheckSocket" @@ d3 :> "CheckSocket"),socket_owner |-> "nobody",socket_exists |-> FALSE,socket_responsive |-> FALSE,running_daemons |-> {}]),
    ([daemon_phase |-> (d1 :> "listening" @@ d2 :> "init" @@ d3 :> "init"),pc |-> (d1 :> "Done" @@ d2 :> "CheckSocket" @@ d3 :> "CheckSocket"),socket_owner |-> d1,socket_exists |-> TRUE,socket_responsive |-> TRUE,running_daemons |-> {d1}]),
    ([daemon_phase |-> (d1 :> "listening" @@ d2 :> "aborted" @@ d3 :> "init"),pc |-> (d1 :> "Done" @@ d2 :> "Done" @@ d3 :> "CheckSocket"),socket_owner |-> d1,socket_exists |-> TRUE,socket_responsive |-> TRUE,running_daemons |-> {d1}]),
    ([daemon_phase |-> (d1 :> "listening" @@ d2 :> "aborted" @@ d3 :> "aborted"),pc |-> (d1 :> "Done" @@ d2 :> "Done" @@ d3 :> "Done"),socket_owner |-> d1,socket_exists |-> TRUE,socket_responsive |-> TRUE,running_daemons |-> {d1}])
    >>
----


=============================================================================

---- MODULE DaemonStartup_TEConstants ----
EXTENDS DaemonStartup

CONSTANTS d1, d2, d3

=============================================================================

---- CONFIG DaemonStartup_TTrace_1772410611 ----
CONSTANTS
    Daemons = { d1 , d2 , d3 }
    d1 = d1
    d2 = d2
    d3 = d3

INVARIANT
    _inv

CHECK_DEADLOCK
    \* CHECK_DEADLOCK off because of PROPERTY or INVARIANT above.
    FALSE

INIT
    _init

NEXT
    _next

CONSTANT
    _TETrace <- _trace

ALIAS
    _expression
=============================================================================
\* Generated on Mon Mar 02 09:16:52 JST 2026