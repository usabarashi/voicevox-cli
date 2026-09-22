#!/usr/bin/env bash
#
# Phase 1 gate for the TLA+ -> Quint migration.
#
# Runs the minimal Quint model with the TLC verification backend and asserts:
#   * the safety and liveness properties hold on the real model, and
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

# Real model: safety and liveness must hold.
run_ok "SynthesisRetry safety" \
  --invariant=attemptsBounded,backoffsBounded,backoffAfterAttempt \
  modeling/quint/SynthesisRetry.qnt
run_ok "SynthesisRetry liveness" \
  --temporal=eventuallyTerminal \
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
echo "Quint verification gate passed (2 positive, 2 negative controls)"
