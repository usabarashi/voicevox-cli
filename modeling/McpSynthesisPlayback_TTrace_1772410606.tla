---- MODULE McpSynthesisPlayback_TTrace_1772410606 ----
EXTENDS Sequences, TLCExt, McpSynthesisPlayback, Toolbox, Naturals, TLC

_expression ==
    LET McpSynthesisPlayback_TEExpression == INSTANCE McpSynthesisPlayback_TEExpression
    IN McpSynthesisPlayback_TEExpression!expression
----

_trace ==
    LET McpSynthesisPlayback_TETrace == INSTANCE McpSynthesisPlayback_TETrace
    IN McpSynthesisPlayback_TETrace!trace
----

_inv ==
    ~(
        TLCGet("level") = Len(_TETrace)
        /\
        user_wants_cancel = (TRUE)
        /\
        pc = ([user |-> "Done_user", server |-> "Done_server", client |-> "Done_client"])
        /\
        audio_location = ("at_client_encoded")
        /\
        client_state = ("cancelled")
        /\
        synthesis_succeeded = (TRUE)
        /\
        server_state = ("idle")
    )
----

_init ==
    /\ synthesis_succeeded = _TETrace[1].synthesis_succeeded
    /\ audio_location = _TETrace[1].audio_location
    /\ client_state = _TETrace[1].client_state
    /\ server_state = _TETrace[1].server_state
    /\ pc = _TETrace[1].pc
    /\ user_wants_cancel = _TETrace[1].user_wants_cancel
----

_next ==
    /\ \E i,j \in DOMAIN _TETrace:
        /\ \/ /\ j = i + 1
              /\ i = TLCGet("level")
        /\ synthesis_succeeded  = _TETrace[i].synthesis_succeeded
        /\ synthesis_succeeded' = _TETrace[j].synthesis_succeeded
        /\ audio_location  = _TETrace[i].audio_location
        /\ audio_location' = _TETrace[j].audio_location
        /\ client_state  = _TETrace[i].client_state
        /\ client_state' = _TETrace[j].client_state
        /\ server_state  = _TETrace[i].server_state
        /\ server_state' = _TETrace[j].server_state
        /\ pc  = _TETrace[i].pc
        /\ pc' = _TETrace[j].pc
        /\ user_wants_cancel  = _TETrace[i].user_wants_cancel
        /\ user_wants_cancel' = _TETrace[j].user_wants_cancel

\* Uncomment the ASSUME below to write the states of the error trace
\* to the given file in Json format. Note that you can pass any tuple
\* to `JsonSerialize`. For example, a sub-sequence of _TETrace.
    \* ASSUME
    \*     LET J == INSTANCE Json
    \*         IN J!JsonSerialize("McpSynthesisPlayback_TTrace_1772410606.json", _TETrace)

=============================================================================

 Note that you can extract this module `McpSynthesisPlayback_TEExpression`
  to a dedicated file to reuse `expression` (the module in the 
  dedicated `McpSynthesisPlayback_TEExpression.tla` file takes precedence 
  over the module `McpSynthesisPlayback_TEExpression` below).

---- MODULE McpSynthesisPlayback_TEExpression ----
EXTENDS Sequences, TLCExt, McpSynthesisPlayback, Toolbox, Naturals, TLC

expression == 
    [
        \* To hide variables of the `McpSynthesisPlayback` spec from the error trace,
        \* remove the variables below.  The trace will be written in the order
        \* of the fields of this record.
        synthesis_succeeded |-> synthesis_succeeded
        ,audio_location |-> audio_location
        ,client_state |-> client_state
        ,server_state |-> server_state
        ,pc |-> pc
        ,user_wants_cancel |-> user_wants_cancel
        
        \* Put additional constant-, state-, and action-level expressions here:
        \* ,_stateNumber |-> _TEPosition
        \* ,_synthesis_succeededUnchanged |-> synthesis_succeeded = synthesis_succeeded'
        
        \* Format the `synthesis_succeeded` variable as Json value.
        \* ,_synthesis_succeededJson |->
        \*     LET J == INSTANCE Json
        \*     IN J!ToJson(synthesis_succeeded)
        
        \* Lastly, you may build expressions over arbitrary sets of states by
        \* leveraging the _TETrace operator.  For example, this is how to
        \* count the number of times a spec variable changed up to the current
        \* state in the trace.
        \* ,_synthesis_succeededModCount |->
        \*     LET F[s \in DOMAIN _TETrace] ==
        \*         IF s = 1 THEN 0
        \*         ELSE IF _TETrace[s].synthesis_succeeded # _TETrace[s-1].synthesis_succeeded
        \*             THEN 1 + F[s-1] ELSE F[s-1]
        \*     IN F[_TEPosition - 1]
    ]

=============================================================================



Parsing and semantic processing can take forever if the trace below is long.
 In this case, it is advised to uncomment the module below to deserialize the
 trace from a generated binary file.

\*
\*---- MODULE McpSynthesisPlayback_TETrace ----
\*EXTENDS IOUtils, McpSynthesisPlayback, TLC
\*
\*trace == IODeserialize("McpSynthesisPlayback_TTrace_1772410606.bin", TRUE)
\*
\*=============================================================================
\*

---- MODULE McpSynthesisPlayback_TETrace ----
EXTENDS McpSynthesisPlayback, TLC

trace == 
    <<
    ([user_wants_cancel |-> FALSE,pc |-> [user |-> "UserAction", server |-> "WaitRequest", client |-> "SendRequest"],audio_location |-> "nowhere",client_state |-> "requesting",synthesis_succeeded |-> FALSE,server_state |-> "idle"]),
    ([user_wants_cancel |-> TRUE,pc |-> [user |-> "Done_user", server |-> "WaitRequest", client |-> "SendRequest"],audio_location |-> "nowhere",client_state |-> "requesting",synthesis_succeeded |-> FALSE,server_state |-> "idle"]),
    ([user_wants_cancel |-> TRUE,pc |-> [user |-> "Done_user", server |-> "WaitRequest", client |-> "ReceiveResponse"],audio_location |-> "nowhere",client_state |-> "waiting",synthesis_succeeded |-> FALSE,server_state |-> "idle"]),
    ([user_wants_cancel |-> TRUE,pc |-> [user |-> "Done_user", server |-> "SynthesizeAction", client |-> "ReceiveResponse"],audio_location |-> "nowhere",client_state |-> "waiting",synthesis_succeeded |-> FALSE,server_state |-> "synthesizing"]),
    ([user_wants_cancel |-> TRUE,pc |-> [user |-> "Done_user", server |-> "Encode", client |-> "ReceiveResponse"],audio_location |-> "at_server_raw",client_state |-> "waiting",synthesis_succeeded |-> TRUE,server_state |-> "synthesizing"]),
    ([user_wants_cancel |-> TRUE,pc |-> [user |-> "Done_user", server |-> "Respond", client |-> "ReceiveResponse"],audio_location |-> "at_server_encoded",client_state |-> "waiting",synthesis_succeeded |-> TRUE,server_state |-> "encoding"]),
    ([user_wants_cancel |-> TRUE,pc |-> [user |-> "Done_user", server |-> "Done_server", client |-> "ReceiveResponse"],audio_location |-> "at_client_encoded",client_state |-> "waiting",synthesis_succeeded |-> TRUE,server_state |-> "idle"]),
    ([user_wants_cancel |-> TRUE,pc |-> [user |-> "Done_user", server |-> "Done_server", client |-> "Done_client"],audio_location |-> "at_client_encoded",client_state |-> "cancelled",synthesis_succeeded |-> TRUE,server_state |-> "idle"])
    >>
----


=============================================================================

---- CONFIG McpSynthesisPlayback_TTrace_1772410606 ----

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
\* Generated on Mon Mar 02 09:16:47 JST 2026