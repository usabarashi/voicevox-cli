------------------------------- MODULE VoicevoxModel ------------------------------
(***************************************************************************)
(* Voice model lifecycle + target existence resolution.                    *)
(* Focus: downloaded/not-downloaded and invalid target behavior.           *)
(***************************************************************************)

EXTENDS Naturals

CONSTANT MAX_RETRY
ASSUME MAX_RETRY \in Nat

VARIABLES modelState, retryCount, targetState, requestState, errorKind

vars == << modelState, retryCount, targetState, requestState, errorKind >>

TypeOK ==
    /\ modelState \in {"NotDownloaded", "Downloading", "Ready", "Failed"}
    /\ retryCount \in 0..MAX_RETRY
    /\ targetState \in {"Unknown", "Exists", "Missing"}
    /\ requestState \in {"Idle", "Requested", "Accepted", "Rejected"}
    /\ errorKind \in {"None", "DownloadFailed", "ModelMissing"}

AcceptedRequiresReadyAndExistingTarget ==
    requestState = "Accepted" => /\ modelState = "Ready"
                                 /\ targetState = "Exists"
                                 /\ errorKind = "None"

RejectedMissingModelHasReason ==
    requestState = "Rejected" => errorKind \in {"DownloadFailed", "ModelMissing"}

Init ==
    /\ modelState = "NotDownloaded"
    /\ retryCount = 0
    /\ targetState = "Unknown"
    /\ requestState = "Idle"
    /\ errorKind = "None"

StartDownload ==
    /\ modelState \in {"NotDownloaded", "Failed"}
    /\ requestState = "Idle"
    /\ modelState' = "Downloading"
    /\ UNCHANGED << retryCount, targetState, requestState, errorKind >>

DownloadOk ==
    /\ modelState = "Downloading"
    /\ requestState = "Idle"
    /\ modelState' = "Ready"
    /\ errorKind' = "None"
    /\ UNCHANGED << retryCount, targetState, requestState >>

DownloadFail ==
    /\ modelState = "Downloading"
    /\ requestState = "Idle"
    /\ modelState' = "Failed"
    /\ retryCount' = IF retryCount < MAX_RETRY THEN retryCount + 1 ELSE retryCount
    /\ errorKind' = "DownloadFailed"
    /\ UNCHANGED << targetState, requestState >>

SetTargetExists ==
    /\ requestState = "Idle"
    /\ targetState' = "Exists"
    /\ UNCHANGED << modelState, retryCount, requestState, errorKind >>

SetTargetMissing ==
    /\ requestState = "Idle"
    /\ targetState' = "Missing"
    /\ UNCHANGED << modelState, retryCount, requestState, errorKind >>

RequestSynthesis ==
    /\ requestState = "Idle"
    /\ targetState # "Unknown"
    /\ requestState' = "Requested"
    /\ UNCHANGED << modelState, retryCount, targetState, errorKind >>

AcceptRequest ==
    /\ requestState = "Requested"
    /\ modelState = "Ready"
    /\ targetState = "Exists"
    /\ requestState' = "Accepted"
    /\ errorKind' = "None"
    /\ UNCHANGED << modelState, retryCount, targetState >>

RejectMissingTarget ==
    /\ requestState = "Requested"
    /\ targetState = "Missing"
    /\ requestState' = "Rejected"
    /\ errorKind' = "ModelMissing"
    /\ UNCHANGED << modelState, retryCount, targetState >>

RejectUndownloadedModel ==
    /\ requestState = "Requested"
    /\ targetState = "Exists"
    /\ modelState # "Ready"
    /\ requestState' = "Rejected"
    /\ errorKind' = "ModelMissing"
    /\ UNCHANGED << modelState, retryCount, targetState >>

ResetRequest ==
    /\ requestState \in {"Accepted", "Rejected"}
    /\ requestState' = "Idle"
    /\ errorKind' = "None"
    /\ UNCHANGED << modelState, retryCount, targetState >>

Stutter ==
    UNCHANGED vars

Next ==
    \/ StartDownload
    \/ DownloadOk
    \/ DownloadFail
    \/ SetTargetExists
    \/ SetTargetMissing
    \/ RequestSynthesis
    \/ AcceptRequest
    \/ RejectMissingTarget
    \/ RejectUndownloadedModel
    \/ ResetRequest
    \/ Stutter

Spec ==
    Init /\ [][Next]_vars

=============================================================================
