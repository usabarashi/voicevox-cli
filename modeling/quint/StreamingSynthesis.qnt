// Streaming synthesis model (default MCP path).
//
// `handle_streaming_synthesis` in
// `src/interface/mcp_server/tools/text_to_speech.rs` is the **default** MCP path
// (`default_streaming()` returns `true`). It does not use the retry loop of
// `SynthesisRetry.qnt`. Instead it:
//   1. connects to the daemon (`connect_with_retry`, no auto-start),
//   2. splits the text (`TextSplitter`) and synthesizes every segment
//      sequentially (`StreamingSynthesizer::request_streaming_synthesis_segments`),
//   3. concatenates the WAV segments (`concatenate_wav_segments`),
//   4. plays the result (unless canceled first).
//
// This module models that lifecycle. A failure at any step is terminal for the
// request, and cancellation before playback prevents playback.
//
// The `scenario*` actions are the coarse, deterministic scenario used by
// `mbt/tests/streaming_synthesis.rs` against a real daemon with a 2-segment
// input; they set the segment counts that the detailed actions produce.

module StreamingSynthesis {
  type Phase = Idle | Connected | Segmenting | Concatenated | Played | Failed | Canceled

  var phase: Phase
  var segmentsSynth: int
  var segmentsTotal: int
  var playbackStarted: bool

  action init = all {
    phase' = Idle,
    segmentsSynth' = 0,
    segmentsTotal' = 0,
    playbackStarted' = false,
  }

  action connect = all {
    phase == Idle,
    phase' = Connected,
    segmentsSynth' = 0,
    segmentsTotal' = 0,
    playbackStarted' = false,
  }

  // `connect_with_retry` can fail before any segment is synthesized.
  action connectFail = all {
    phase == Idle,
    phase' = Failed,
    segmentsSynth' = 0,
    segmentsTotal' = 0,
    playbackStarted' = false,
  }

  // The text is split into at least one non-empty segment.
  action split = all {
    phase == Connected,
    nondet n = 1.to(3).oneOf()
    all {
      phase' = Segmenting,
      segmentsSynth' = 0,
      segmentsTotal' = n,
      playbackStarted' = false,
    }
  }

  action segmentOk = all {
    phase == Segmenting,
    segmentsSynth < segmentsTotal,
    phase' = Segmenting,
    segmentsSynth' = segmentsSynth + 1,
    segmentsTotal' = segmentsTotal,
    playbackStarted' = false,
  }

  action segmentFail = all {
    phase == Segmenting,
    phase' = Failed,
    segmentsSynth' = segmentsSynth,
    segmentsTotal' = segmentsTotal,
    playbackStarted' = false,
  }

  action concatenate = all {
    phase == Segmenting,
    segmentsTotal >= 1,
    segmentsSynth == segmentsTotal,
    phase' = Concatenated,
    segmentsSynth' = segmentsSynth,
    segmentsTotal' = segmentsTotal,
    playbackStarted' = false,
  }

  action concatenateFail = all {
    phase == Segmenting,
    segmentsSynth == segmentsTotal,
    phase' = Failed,
    segmentsSynth' = segmentsSynth,
    segmentsTotal' = segmentsTotal,
    playbackStarted' = false,
  }

  action play = all {
    phase == Concatenated,
    phase' = Played,
    segmentsSynth' = segmentsSynth,
    segmentsTotal' = segmentsTotal,
    playbackStarted' = true,
  }

  // Cancellation can arrive at any point before playback, including before the
  // connection is established (production checks a pre-delivered cancellation
  // first and races cancellation against the whole synthesis future).
  action cancel = all {
    phase == Idle or phase == Connected or phase == Segmenting or phase == Concatenated,
    phase' = Canceled,
    segmentsSynth' = segmentsSynth,
    segmentsTotal' = segmentsTotal,
    playbackStarted' = false,
  }

  action stutter = all {
    phase' = phase,
    segmentsSynth' = segmentsSynth,
    segmentsTotal' = segmentsTotal,
    playbackStarted' = playbackStarted,
  }

  action step = any {
    connect,
    connectFail,
    split,
    segmentOk,
    segmentFail,
    concatenate,
    concatenateFail,
    play,
    cancel,
    stutter,
  }

  // ---- Properties ---------------------------------------------------------

  val segmentsBounded = segmentsSynth >= 0 and segmentsSynth <= segmentsTotal

  val playbackRequiresAllSegments =
    playbackStarted implies (segmentsTotal >= 1 and segmentsSynth == segmentsTotal)

  // Once canceled, playback never starts.
  val canceledImpliesNoPlayback = phase == Canceled implies not(playbackStarted)

  // ---- Fixed scenario (real-daemon MBT) -----------------------------------
  //
  // A 2-segment input, synthesized and concatenated without playback.

  action scenarioConnect = all {
    phase' = Connected,
    segmentsSynth' = 0,
    segmentsTotal' = 0,
    playbackStarted' = false,
  }

  action scenarioSynthesizeSegments = all {
    phase == Connected,
    phase' = Segmenting,
    segmentsSynth' = 2,
    segmentsTotal' = 2,
    playbackStarted' = false,
  }

  action scenarioConcatenate = all {
    phase == Segmenting,
    phase' = Concatenated,
    segmentsSynth' = 2,
    segmentsTotal' = 2,
    playbackStarted' = false,
  }

  action scenarioStutter = all {
    phase == Concatenated,
    phase' = phase,
    segmentsSynth' = segmentsSynth,
    segmentsTotal' = segmentsTotal,
    playbackStarted' = playbackStarted,
  }

  action scenarioStep = any {
    scenarioSynthesizeSegments,
    scenarioConcatenate,
    scenarioStutter,
  }
}
