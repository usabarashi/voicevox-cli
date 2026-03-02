---- MODULE DaemonRequestHandling_TTrace_1772410608 ----
EXTENDS DaemonRequestHandling, Sequences, TLCExt, DaemonRequestHandling_TEConstants, Toolbox, Naturals, TLC

_expression ==
    LET DaemonRequestHandling_TEExpression == INSTANCE DaemonRequestHandling_TEExpression
    IN DaemonRequestHandling_TEExpression!expression
----

_trace ==
    LET DaemonRequestHandling_TETrace == INSTANCE DaemonRequestHandling_TETrace
    IN DaemonRequestHandling_TETrace!trace
----

_inv ==
    ~(
        TLCGet("level") = Len(_TETrace)
        /\
        pc = ((c1 :> "Done" @@ c2 :> "Done" @@ c3 :> "Done" @@ c4 :> "Done"))
        /\
        request_type = ((c1 :> "ListSpeakers" @@ c2 :> "ListSpeakers" @@ c3 :> "ListSpeakers" @@ c4 :> "ListSpeakers"))
        /\
        client_state = ((c1 :> "done" @@ c2 :> "done" @@ c3 :> "done" @@ c4 :> "done"))
        /\
        response = ((c1 :> "ok" @@ c2 :> "ok" @@ c3 :> "ok" @@ c4 :> "ok"))
        /\
        semaphore = (3)
        /\
        synthesis_outcome = ((c1 :> "success" @@ c2 :> "success" @@ c3 :> "success" @@ c4 :> "success"))
        /\
        model_loaded = (FALSE)
        /\
        mutex_holder = ("nobody")
    )
----

_init ==
    /\ mutex_holder = _TETrace[1].mutex_holder
    /\ semaphore = _TETrace[1].semaphore
    /\ client_state = _TETrace[1].client_state
    /\ synthesis_outcome = _TETrace[1].synthesis_outcome
    /\ response = _TETrace[1].response
    /\ pc = _TETrace[1].pc
    /\ request_type = _TETrace[1].request_type
    /\ model_loaded = _TETrace[1].model_loaded
----

_next ==
    /\ \E i,j \in DOMAIN _TETrace:
        /\ \/ /\ j = i + 1
              /\ i = TLCGet("level")
        /\ mutex_holder  = _TETrace[i].mutex_holder
        /\ mutex_holder' = _TETrace[j].mutex_holder
        /\ semaphore  = _TETrace[i].semaphore
        /\ semaphore' = _TETrace[j].semaphore
        /\ client_state  = _TETrace[i].client_state
        /\ client_state' = _TETrace[j].client_state
        /\ synthesis_outcome  = _TETrace[i].synthesis_outcome
        /\ synthesis_outcome' = _TETrace[j].synthesis_outcome
        /\ response  = _TETrace[i].response
        /\ response' = _TETrace[j].response
        /\ pc  = _TETrace[i].pc
        /\ pc' = _TETrace[j].pc
        /\ request_type  = _TETrace[i].request_type
        /\ request_type' = _TETrace[j].request_type
        /\ model_loaded  = _TETrace[i].model_loaded
        /\ model_loaded' = _TETrace[j].model_loaded

\* Uncomment the ASSUME below to write the states of the error trace
\* to the given file in Json format. Note that you can pass any tuple
\* to `JsonSerialize`. For example, a sub-sequence of _TETrace.
    \* ASSUME
    \*     LET J == INSTANCE Json
    \*         IN J!JsonSerialize("DaemonRequestHandling_TTrace_1772410608.json", _TETrace)

=============================================================================

 Note that you can extract this module `DaemonRequestHandling_TEExpression`
  to a dedicated file to reuse `expression` (the module in the 
  dedicated `DaemonRequestHandling_TEExpression.tla` file takes precedence 
  over the module `DaemonRequestHandling_TEExpression` below).

---- MODULE DaemonRequestHandling_TEExpression ----
EXTENDS DaemonRequestHandling, Sequences, TLCExt, DaemonRequestHandling_TEConstants, Toolbox, Naturals, TLC

expression == 
    [
        \* To hide variables of the `DaemonRequestHandling` spec from the error trace,
        \* remove the variables below.  The trace will be written in the order
        \* of the fields of this record.
        mutex_holder |-> mutex_holder
        ,semaphore |-> semaphore
        ,client_state |-> client_state
        ,synthesis_outcome |-> synthesis_outcome
        ,response |-> response
        ,pc |-> pc
        ,request_type |-> request_type
        ,model_loaded |-> model_loaded
        
        \* Put additional constant-, state-, and action-level expressions here:
        \* ,_stateNumber |-> _TEPosition
        \* ,_mutex_holderUnchanged |-> mutex_holder = mutex_holder'
        
        \* Format the `mutex_holder` variable as Json value.
        \* ,_mutex_holderJson |->
        \*     LET J == INSTANCE Json
        \*     IN J!ToJson(mutex_holder)
        
        \* Lastly, you may build expressions over arbitrary sets of states by
        \* leveraging the _TETrace operator.  For example, this is how to
        \* count the number of times a spec variable changed up to the current
        \* state in the trace.
        \* ,_mutex_holderModCount |->
        \*     LET F[s \in DOMAIN _TETrace] ==
        \*         IF s = 1 THEN 0
        \*         ELSE IF _TETrace[s].mutex_holder # _TETrace[s-1].mutex_holder
        \*             THEN 1 + F[s-1] ELSE F[s-1]
        \*     IN F[_TEPosition - 1]
    ]

=============================================================================



Parsing and semantic processing can take forever if the trace below is long.
 In this case, it is advised to uncomment the module below to deserialize the
 trace from a generated binary file.

\*
\*---- MODULE DaemonRequestHandling_TETrace ----
\*EXTENDS DaemonRequestHandling, IOUtils, DaemonRequestHandling_TEConstants, TLC
\*
\*trace == IODeserialize("DaemonRequestHandling_TTrace_1772410608.bin", TRUE)
\*
\*=============================================================================
\*

---- MODULE DaemonRequestHandling_TETrace ----
EXTENDS DaemonRequestHandling, DaemonRequestHandling_TEConstants, TLC

trace == 
    <<
    ([pc |-> (c1 :> "AcquirePermit" @@ c2 :> "AcquirePermit" @@ c3 :> "AcquirePermit" @@ c4 :> "AcquirePermit"),request_type |-> (c1 :> "Synthesize" @@ c2 :> "Synthesize" @@ c3 :> "Synthesize" @@ c4 :> "Synthesize"),client_state |-> (c1 :> "idle" @@ c2 :> "idle" @@ c3 :> "idle" @@ c4 :> "idle"),response |-> (c1 :> "none" @@ c2 :> "none" @@ c3 :> "none" @@ c4 :> "none"),semaphore |-> 3,synthesis_outcome |-> (c1 :> "success" @@ c2 :> "success" @@ c3 :> "success" @@ c4 :> "success"),model_loaded |-> FALSE,mutex_holder |-> "nobody"]),
    ([pc |-> (c1 :> "ChooseRequest" @@ c2 :> "AcquirePermit" @@ c3 :> "AcquirePermit" @@ c4 :> "AcquirePermit"),request_type |-> (c1 :> "Synthesize" @@ c2 :> "Synthesize" @@ c3 :> "Synthesize" @@ c4 :> "Synthesize"),client_state |-> (c1 :> "has_permit" @@ c2 :> "idle" @@ c3 :> "idle" @@ c4 :> "idle"),response |-> (c1 :> "none" @@ c2 :> "none" @@ c3 :> "none" @@ c4 :> "none"),semaphore |-> 2,synthesis_outcome |-> (c1 :> "success" @@ c2 :> "success" @@ c3 :> "success" @@ c4 :> "success"),model_loaded |-> FALSE,mutex_holder |-> "nobody"]),
    ([pc |-> (c1 :> "SendResponse" @@ c2 :> "AcquirePermit" @@ c3 :> "AcquirePermit" @@ c4 :> "AcquirePermit"),request_type |-> (c1 :> "ListSpeakers" @@ c2 :> "Synthesize" @@ c3 :> "Synthesize" @@ c4 :> "Synthesize"),client_state |-> (c1 :> "responding" @@ c2 :> "idle" @@ c3 :> "idle" @@ c4 :> "idle"),response |-> (c1 :> "ok" @@ c2 :> "none" @@ c3 :> "none" @@ c4 :> "none"),semaphore |-> 2,synthesis_outcome |-> (c1 :> "success" @@ c2 :> "success" @@ c3 :> "success" @@ c4 :> "success"),model_loaded |-> FALSE,mutex_holder |-> "nobody"]),
    ([pc |-> (c1 :> "Done" @@ c2 :> "AcquirePermit" @@ c3 :> "AcquirePermit" @@ c4 :> "AcquirePermit"),request_type |-> (c1 :> "ListSpeakers" @@ c2 :> "Synthesize" @@ c3 :> "Synthesize" @@ c4 :> "Synthesize"),client_state |-> (c1 :> "done" @@ c2 :> "idle" @@ c3 :> "idle" @@ c4 :> "idle"),response |-> (c1 :> "ok" @@ c2 :> "none" @@ c3 :> "none" @@ c4 :> "none"),semaphore |-> 3,synthesis_outcome |-> (c1 :> "success" @@ c2 :> "success" @@ c3 :> "success" @@ c4 :> "success"),model_loaded |-> FALSE,mutex_holder |-> "nobody"]),
    ([pc |-> (c1 :> "Done" @@ c2 :> "ChooseRequest" @@ c3 :> "AcquirePermit" @@ c4 :> "AcquirePermit"),request_type |-> (c1 :> "ListSpeakers" @@ c2 :> "Synthesize" @@ c3 :> "Synthesize" @@ c4 :> "Synthesize"),client_state |-> (c1 :> "done" @@ c2 :> "has_permit" @@ c3 :> "idle" @@ c4 :> "idle"),response |-> (c1 :> "ok" @@ c2 :> "none" @@ c3 :> "none" @@ c4 :> "none"),semaphore |-> 2,synthesis_outcome |-> (c1 :> "success" @@ c2 :> "success" @@ c3 :> "success" @@ c4 :> "success"),model_loaded |-> FALSE,mutex_holder |-> "nobody"]),
    ([pc |-> (c1 :> "Done" @@ c2 :> "SendResponse" @@ c3 :> "AcquirePermit" @@ c4 :> "AcquirePermit"),request_type |-> (c1 :> "ListSpeakers" @@ c2 :> "ListSpeakers" @@ c3 :> "Synthesize" @@ c4 :> "Synthesize"),client_state |-> (c1 :> "done" @@ c2 :> "responding" @@ c3 :> "idle" @@ c4 :> "idle"),response |-> (c1 :> "ok" @@ c2 :> "ok" @@ c3 :> "none" @@ c4 :> "none"),semaphore |-> 2,synthesis_outcome |-> (c1 :> "success" @@ c2 :> "success" @@ c3 :> "success" @@ c4 :> "success"),model_loaded |-> FALSE,mutex_holder |-> "nobody"]),
    ([pc |-> (c1 :> "Done" @@ c2 :> "Done" @@ c3 :> "AcquirePermit" @@ c4 :> "AcquirePermit"),request_type |-> (c1 :> "ListSpeakers" @@ c2 :> "ListSpeakers" @@ c3 :> "Synthesize" @@ c4 :> "Synthesize"),client_state |-> (c1 :> "done" @@ c2 :> "done" @@ c3 :> "idle" @@ c4 :> "idle"),response |-> (c1 :> "ok" @@ c2 :> "ok" @@ c3 :> "none" @@ c4 :> "none"),semaphore |-> 3,synthesis_outcome |-> (c1 :> "success" @@ c2 :> "success" @@ c3 :> "success" @@ c4 :> "success"),model_loaded |-> FALSE,mutex_holder |-> "nobody"]),
    ([pc |-> (c1 :> "Done" @@ c2 :> "Done" @@ c3 :> "ChooseRequest" @@ c4 :> "AcquirePermit"),request_type |-> (c1 :> "ListSpeakers" @@ c2 :> "ListSpeakers" @@ c3 :> "Synthesize" @@ c4 :> "Synthesize"),client_state |-> (c1 :> "done" @@ c2 :> "done" @@ c3 :> "has_permit" @@ c4 :> "idle"),response |-> (c1 :> "ok" @@ c2 :> "ok" @@ c3 :> "none" @@ c4 :> "none"),semaphore |-> 2,synthesis_outcome |-> (c1 :> "success" @@ c2 :> "success" @@ c3 :> "success" @@ c4 :> "success"),model_loaded |-> FALSE,mutex_holder |-> "nobody"]),
    ([pc |-> (c1 :> "Done" @@ c2 :> "Done" @@ c3 :> "SendResponse" @@ c4 :> "AcquirePermit"),request_type |-> (c1 :> "ListSpeakers" @@ c2 :> "ListSpeakers" @@ c3 :> "ListSpeakers" @@ c4 :> "Synthesize"),client_state |-> (c1 :> "done" @@ c2 :> "done" @@ c3 :> "responding" @@ c4 :> "idle"),response |-> (c1 :> "ok" @@ c2 :> "ok" @@ c3 :> "ok" @@ c4 :> "none"),semaphore |-> 2,synthesis_outcome |-> (c1 :> "success" @@ c2 :> "success" @@ c3 :> "success" @@ c4 :> "success"),model_loaded |-> FALSE,mutex_holder |-> "nobody"]),
    ([pc |-> (c1 :> "Done" @@ c2 :> "Done" @@ c3 :> "Done" @@ c4 :> "AcquirePermit"),request_type |-> (c1 :> "ListSpeakers" @@ c2 :> "ListSpeakers" @@ c3 :> "ListSpeakers" @@ c4 :> "Synthesize"),client_state |-> (c1 :> "done" @@ c2 :> "done" @@ c3 :> "done" @@ c4 :> "idle"),response |-> (c1 :> "ok" @@ c2 :> "ok" @@ c3 :> "ok" @@ c4 :> "none"),semaphore |-> 3,synthesis_outcome |-> (c1 :> "success" @@ c2 :> "success" @@ c3 :> "success" @@ c4 :> "success"),model_loaded |-> FALSE,mutex_holder |-> "nobody"]),
    ([pc |-> (c1 :> "Done" @@ c2 :> "Done" @@ c3 :> "Done" @@ c4 :> "ChooseRequest"),request_type |-> (c1 :> "ListSpeakers" @@ c2 :> "ListSpeakers" @@ c3 :> "ListSpeakers" @@ c4 :> "Synthesize"),client_state |-> (c1 :> "done" @@ c2 :> "done" @@ c3 :> "done" @@ c4 :> "has_permit"),response |-> (c1 :> "ok" @@ c2 :> "ok" @@ c3 :> "ok" @@ c4 :> "none"),semaphore |-> 2,synthesis_outcome |-> (c1 :> "success" @@ c2 :> "success" @@ c3 :> "success" @@ c4 :> "success"),model_loaded |-> FALSE,mutex_holder |-> "nobody"]),
    ([pc |-> (c1 :> "Done" @@ c2 :> "Done" @@ c3 :> "Done" @@ c4 :> "SendResponse"),request_type |-> (c1 :> "ListSpeakers" @@ c2 :> "ListSpeakers" @@ c3 :> "ListSpeakers" @@ c4 :> "ListSpeakers"),client_state |-> (c1 :> "done" @@ c2 :> "done" @@ c3 :> "done" @@ c4 :> "responding"),response |-> (c1 :> "ok" @@ c2 :> "ok" @@ c3 :> "ok" @@ c4 :> "ok"),semaphore |-> 2,synthesis_outcome |-> (c1 :> "success" @@ c2 :> "success" @@ c3 :> "success" @@ c4 :> "success"),model_loaded |-> FALSE,mutex_holder |-> "nobody"]),
    ([pc |-> (c1 :> "Done" @@ c2 :> "Done" @@ c3 :> "Done" @@ c4 :> "Done"),request_type |-> (c1 :> "ListSpeakers" @@ c2 :> "ListSpeakers" @@ c3 :> "ListSpeakers" @@ c4 :> "ListSpeakers"),client_state |-> (c1 :> "done" @@ c2 :> "done" @@ c3 :> "done" @@ c4 :> "done"),response |-> (c1 :> "ok" @@ c2 :> "ok" @@ c3 :> "ok" @@ c4 :> "ok"),semaphore |-> 3,synthesis_outcome |-> (c1 :> "success" @@ c2 :> "success" @@ c3 :> "success" @@ c4 :> "success"),model_loaded |-> FALSE,mutex_holder |-> "nobody"])
    >>
----


=============================================================================

---- MODULE DaemonRequestHandling_TEConstants ----
EXTENDS DaemonRequestHandling

CONSTANTS c1, c2, c3, c4

=============================================================================

---- CONFIG DaemonRequestHandling_TTrace_1772410608 ----
CONSTANTS
    MAX_CLIENTS = 3
    Clients = { c1 , c2 , c3 , c4 }
    c2 = c2
    c3 = c3
    c1 = c1
    c4 = c4

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
\* Generated on Mon Mar 02 09:16:49 JST 2026