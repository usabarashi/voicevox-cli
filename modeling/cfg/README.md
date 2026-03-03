# TLA+ Config Scenarios

This directory contains TLC configuration files grouped by scenario intent.
Use these names as the primary entry points for model checking.

## Startup and readiness

- `Daemon.startup.cfg`
- `FirstStartup.bootstrap.cfg`
- `ONNXRuntime.load.cfg`
- `VoicevoxModel.standard.cfg`
- `Dictionary.load.cfg`
- `Socket.bind.cfg`

## Client and IPC

- `MCPServer.connect.cfg`
- `MCPServer.degraded.cfg`
- `IPC.safety.cfg`
- `IPC.progress.cfg`

## Synthesis

- `Synthesis.full.cfg`
- `Synthesis.normal-flow.cfg`
- `Synthesis.invalid-target.cfg`
- `Synthesis.progress.cfg`
- `Synthesis.retry-boundary.cfg`
- `Synthesis.cancel-sources.cfg`
- `SynthesisParallel.safety.cfg`
- `SynthesisParallel.progress.cfg`

## Playback and integrated system

- `Playback.standard.cfg`
- `Say.standard.cfg`
- `Say.daemon.cfg`
- `System.integration.cfg`

## Notes

- Existing formulas and constants are unchanged; this rename is naming-only.
- Pair each config with its corresponding module in `modeling/tla/`.

## TLC command examples

Run from repository root.

```bash
# Startup
tlc -config modeling/cfg/Daemon.startup.cfg modeling/tla/Daemon.tla
tlc -config modeling/cfg/FirstStartup.bootstrap.cfg modeling/tla/StartupResources.tla
tlc -config modeling/cfg/ONNXRuntime.load.cfg modeling/tla/ONNXRuntime.tla
tlc -config modeling/cfg/VoicevoxModel.standard.cfg modeling/tla/VoicevoxModel.tla

# Client / IPC
tlc -config modeling/cfg/MCPServer.connect.cfg modeling/tla/MCPServer.tla
tlc -config modeling/cfg/MCPServer.degraded.cfg modeling/tla/MCPServer.tla
tlc -config modeling/cfg/IPC.safety.cfg modeling/tla/IPC.tla
tlc -config modeling/cfg/IPC.progress.cfg modeling/tla/IPC.tla

# Synthesis
tlc -config modeling/cfg/Synthesis.full.cfg modeling/tla/Synthesis.tla
tlc -config modeling/cfg/Synthesis.normal-flow.cfg modeling/tla/Synthesis.tla
tlc -config modeling/cfg/Synthesis.invalid-target.cfg modeling/tla/Synthesis.tla
tlc -config modeling/cfg/Synthesis.progress.cfg modeling/tla/Synthesis.tla
tlc -config modeling/cfg/Synthesis.retry-boundary.cfg modeling/tla/Synthesis.tla
tlc -config modeling/cfg/Synthesis.cancel-sources.cfg modeling/tla/Synthesis.tla
tlc -config modeling/cfg/SynthesisParallel.safety.cfg modeling/tla/SynthesisParallel.tla
tlc -config modeling/cfg/SynthesisParallel.progress.cfg modeling/tla/SynthesisParallel.tla

# Playback / Integrated system
tlc -config modeling/cfg/Playback.standard.cfg modeling/tla/Playback.tla
tlc -config modeling/cfg/Say.standard.cfg modeling/tla/Say.tla
tlc -config modeling/cfg/Say.daemon.cfg modeling/tla/Say.tla
tlc -config modeling/cfg/System.integration.cfg modeling/tla/System.tla
```
