#!/usr/bin/env bash
#
# Quint verification gate for the TLA+ -> Quint migration.
#
# Runs the Quint models with the TLC verification backend and asserts:
#   * the safety and liveness properties hold on the real models, and
#   * the negative controls are detected for the intended reason
#     (safety violation / infinite stall, not just a deadlock).
#
# Requires `quint` on PATH (provided by the Nix devShell, see flake.nix).

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${here}/../.." && pwd)"
cd "${repo_root}"

if ! command -v quint >/dev/null 2>&1; then
  echo "error: 'quint' not found on PATH (run inside 'nix develop')" >&2
  exit 1
fi

max_steps=12
failures=0

# Expect verification to succeed.
run_ok() {
  local desc="$1"
  shift
  echo "::group::${desc}"
  if quint verify --backend=tlc --max-steps="${max_steps}" "$@"; then
    echo "ok: ${desc}"
  else
    echo "FAIL: expected verification to pass: ${desc}" >&2
    failures=$((failures + 1))
  fi
  echo "::endgroup::"
}

# Every spec must at least parse and typecheck.
check_typecheck() {
  local spec="$1"
  echo "::group::typecheck ${spec}"
  if quint typecheck "${spec}"; then
    echo "ok: typecheck ${spec}"
  else
    echo "FAIL: typecheck failed: ${spec}" >&2
    failures=$((failures + 1))
  fi
  echo "::endgroup::"
}

# Expect verification to fail with a counterexample ("violation").
run_expect_violation() {
  local desc="$1"
  shift
  echo "::group::${desc} (expect violation)"
  local output status
  set +e
  output="$(quint verify --backend=tlc --max-steps="${max_steps}" "$@" 2>&1)"
  status=$?
  set -e
  echo "${output}"
  if [ "${status}" -eq 0 ]; then
    echo "FAIL: expected a violation but verification passed: ${desc}" >&2
    failures=$((failures + 1))
  elif ! grep -q "violation" <<<"${output}"; then
    echo "FAIL: expected a violation report, but got a different error: ${desc}" >&2
    failures=$((failures + 1))
  else
    echo "ok: detected violation as intended: ${desc}"
  fi
  echo "::endgroup::"
}

# Every spec must be classified as MBT-backed or verified-only, and every
# MBT-backed spec must have a driver that references it (see MODEL_CLASSIFICATION).
# Shared constants must agree across specs (see EXPECTED_CONSTANTS). Catches
# cross-spec drift such as a connect budget changed in one spec only.
check_constant_consistency() {
  local file="modeling/quint/EXPECTED_CONSTANTS"
  echo "::group::constant consistency"
  local kind spec name value
  while read -r kind spec name value; do
    case "${kind}" in
      ""|\#*) continue ;;
      spec)
        if ! grep -qE "pure val ${name} = ${value}([^0-9]|$)" "modeling/quint/${spec}.qnt"; then
          echo "FAIL: ${spec}.qnt does not declare 'pure val ${name} = ${value}'" >&2
          failures=$((failures + 1))
        fi
        ;;
      *)
        echo "FAIL: unknown entry kind '${kind}' in ${file}" >&2
        failures=$((failures + 1))
        ;;
    esac
  done < "${file}"
  echo "ok: constant consistency"
  echo "::endgroup::"
}

check_model_classification() {
  local class_file="modeling/quint/MODEL_CLASSIFICATION"
  echo "::group::model classification"
  local spec name tier
  for spec in modeling/quint/*.qnt; do
    name="$(basename "${spec}" .qnt)"
    if ! grep -qE "^${name}[[:space:]]" "${class_file}"; then
      echo "FAIL: spec ${name} is not classified in ${class_file}" >&2
      failures=$((failures + 1))
    fi
  done

  while read -r name tier; do
    case "${name}" in
      ""|\#*) continue ;;
    esac
    if [ "${tier}" = "mbt" ]; then
      if ! grep -rq "modeling/quint/${name}.qnt" mbt/tests; then
        echo "FAIL: ${name} is classified 'mbt' but no driver references it" >&2
        failures=$((failures + 1))
      fi
    elif [ "${tier}" != "verified-only" ]; then
      echo "FAIL: unknown tier '${tier}' for ${name} in ${class_file}" >&2
      failures=$((failures + 1))
    fi
  done < "${class_file}"
  echo "ok: model classification"
  echo "::endgroup::"
}

# All specs must parse and typecheck.
for spec in modeling/quint/*.qnt modeling/quint/negative/*.qnt; do
  check_typecheck "${spec}"
done

check_model_classification
check_constant_consistency

# Ported lifecycle models (Phase 2).
run_ok "ONNXRuntime liveness" \
  --temporal=loadTerminates \
  modeling/quint/ONNXRuntime.qnt
run_ok "ONNXRuntime stability" \
  --temporal=readyIsStable \
  modeling/quint/ONNXRuntime.qnt
run_ok "Dictionary liveness" \
  --temporal=loadTerminates \
  modeling/quint/Dictionary.qnt
run_ok "Dictionary stability" \
  --temporal=loadedStaysReady \
  modeling/quint/Dictionary.qnt
run_ok "Socket liveness" \
  --temporal=bindingTerminates \
  modeling/quint/Socket.qnt
run_ok "Socket denial terminal" \
  --temporal=permissionDeniedIsTerminal \
  modeling/quint/Socket.qnt
run_ok "Playback safety" \
  --invariant=playingRequiresAudio,canceledImpliesStoppedOrFailed \
  modeling/quint/Playback.qnt
run_ok "IPC safety" \
  --invariant=failedImpliesError,doneImpliesValidResponse,inFlightHasNoError \
  modeling/quint/IPC.qnt
run_ok "IPC progress" \
  --temporal=eventuallyLeavesInFlight \
  modeling/quint/IPC.qnt
run_ok "Daemon safety" \
  --invariant=typeOK,socketImpliesReady,busyImpliesReady,retryBounded,alreadyRunningNotBusy \
  modeling/quint/Daemon.qnt
run_ok "DaemonSynthesisPath safety" \
  --invariant=typeOK,inFlightMatchesHolding,atMostOneSynthesizing,workerBusyMatchesSynthesizing \
  modeling/quint/DaemonSynthesisPath.qnt
run_ok "DaemonSynthesisPath liveness" \
  --temporal=workerEventuallyIdle \
  modeling/quint/DaemonSynthesisPath.qnt
run_ok "Daemon serialization safety" \
  --invariant=atMostOneSynthesizing,workerMatchesSynthesis \
  modeling/quint/DaemonSerialization.qnt
run_ok "Daemon serialization progress" \
  --temporal=eventuallyLeavesBusyWorker \
  modeling/quint/DaemonSerialization.qnt
run_ok "StartupResources safety" \
  --invariant=typeOK,daemonReadyRequiresDownloads,daemonStartRequiresDownloads,daemonReadyRequiresSocket \
  modeling/quint/StartupResources.qnt
run_ok "MCPServer safety" \
  --invariant=typeOK,connectedImpliesDaemonReady,playingRequiresAudio \
  modeling/quint/MCPServer.qnt
run_ok "Say safety" \
  --invariant=typeOK,synthesizingImpliesBusyReq,busyReqOwnedBySay,doneHasNoError,playbackFailureOnlyInPlayMode,playingRequiresAudio,emittingUsesPlayMode,outputFailureOnlyInFileMode \
  modeling/quint/Say.qnt
run_ok "System integration" \
  --invariant=typeOK,viewsAligned,clientConnectedImpliesDaemonReady \
  modeling/quint/System.qnt
run_ok "StreamingSynthesis safety" \
  --invariant=segmentsBounded,playbackRequiresAllSegments,canceledImpliesNoPlayback \
  modeling/quint/StreamingSynthesis.qnt
run_ok "Download safety" \
  --invariant=attemptsBounded,failedHasReason,exhaustedImpliesAttempts,preparationFailureMeansNoAttempts \
  modeling/quint/Download.qnt
run_ok "Download liveness" \
  --temporal=terminates \
  modeling/quint/Download.qnt
run_ok "ModelLifecycle safety" \
  --invariant=loadedImpliesPhase \
  modeling/quint/ModelLifecycle.qnt
run_ok "ModelLifecycle liveness" \
  --temporal=eventuallyUnloaded \
  modeling/quint/ModelLifecycle.qnt
run_ok "McpStartup safety" \
  --invariant=doneHasOutcome,recoveryOnlyAfterAlreadyRunning \
  modeling/quint/McpStartup.qnt
run_ok "McpStartup liveness" \
  --temporal=terminates \
  modeling/quint/McpStartup.qnt
run_ok "McpRequestLifecycle safety" \
  --invariant=typeOK,activeMatchesRunning \
  modeling/quint/McpRequestLifecycle.qnt
run_ok "McpRequestLifecycle liveness" \
  --temporal=allRequestsTerminate \
  modeling/quint/McpRequestLifecycle.qnt
run_ok "DaemonServer safety" \
  --invariant=typeOK,inFlightMatchesHandling \
  modeling/quint/DaemonServer.qnt
run_ok "DaemonServer liveness (client 0)" \
  --temporal=handling0Terminates \
  modeling/quint/DaemonServer.qnt
run_ok "DaemonServer liveness (client 1)" \
  --temporal=handling1Terminates \
  modeling/quint/DaemonServer.qnt
run_ok "DaemonServer liveness (client 2)" \
  --temporal=handling2Terminates \
  modeling/quint/DaemonServer.qnt
run_ok "StartupSafety safety" \
  --invariant=readyRequiresResources,readyRequiresSocketReady,startedImpliesNoLive,staleRemovedBeforeStart,removedOnlyStale,alreadyRunningOnlyLive,failedImpliesResourceFailure \
  modeling/quint/StartupSafety.qnt
run_ok "StartupSafety liveness" \
  --temporal=terminates \
  modeling/quint/StartupSafety.qnt
run_ok "DaemonStartup safety" \
  --invariant=liveNeverRemoved,liveNeverStarted,removedOnlyStale,staleRemovedBeforeStart \
  modeling/quint/DaemonStartup.qnt
run_ok "DaemonStartup liveness" \
  --temporal=decides \
  modeling/quint/DaemonStartup.qnt
run_ok "DaemonIpc safety" \
  --invariant=catalogRequiresConnection \
  modeling/quint/DaemonIpc.qnt
run_ok "DaemonSynthesize safety" \
  --invariant=synthesizedRequiresCatalog \
  modeling/quint/DaemonSynthesize.qnt

# Real model: safety and liveness must hold.
run_ok "SynthesisRetry safety" \
  --invariant=attemptsBounded,backoffsBounded,backoffAfterAttempt \
  modeling/quint/SynthesisRetry.qnt
run_ok "SynthesisRetry liveness" \
  --temporal=eventuallyTerminal \
  modeling/quint/SynthesisRetry.qnt
run_ok "SynthesisRetry cancellation is terminal" \
  --temporal=cancelIsTerminal \
  modeling/quint/SynthesisRetry.qnt

# Negative controls: the checker must actually catch violations.
run_expect_violation "SafetyViolation (unbounded attempts)" \
  --invariant=attemptsBounded \
  modeling/quint/negative/SafetyViolation.qnt
run_expect_violation "LivenessViolation (infinite stall, no fairness)" \
  --temporal=eventuallyTerminal \
  modeling/quint/negative/LivenessViolation.qnt

echo
if [ "${failures}" -ne 0 ]; then
  echo "Quint verification gate FAILED (${failures} failure(s))" >&2
  exit 1
fi
echo "Quint verification gate passed"
