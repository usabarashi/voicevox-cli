Migrate the MCP server to the stateless 2026-07-28 revision

## Why

The server implemented the 2025-06-18 revision, where a client opens with `initialize` and the negotiated version and capabilities live in connection state. MCP 2026-07-28 removes that handshake: every request declares its own version and capabilities, and `server/discover` becomes mandatory. Reading the new transport rules also exposed two latent stdio violations worth fixing in the same change — informational logging could land on stdout between frames, and a request the client had cancelled could still be answered.

## What

- Take the stateless revision only, instead of the dual-era mode the spec permits. The CLI, daemon and MCP server ship as a matched set and legacy clients are explicitly not a requirement, so a second code path through the server (legacy session semantics per connection) would buy nothing and double the states to model. A legacy client still receives an error that names the supported versions, which is all a modern-only server can offer it.
- Put version and capability validation in the request parser rather than in each method handler, and keep "malformed" separate from "unsupported version": the first is `-32602` because nothing can be retried, the second is `-32022` carrying `requested`/`supported` because the client is expected to retry. That distinction is the part that changes client behaviour, so it is also the part the parsing model and its MBT driver assert.
- Reject `initialize` before validating `_meta`. Legacy clients by definition send no `_meta`, so checking it first would answer them with a generic params error instead of the version list they need in order to explain the failure to a user.
- Treat cancellation as a commit-point problem instead of best-effort. Writer capacity is reserved before the registration is retired and the frame is enqueued while the registry lock is held, and frames still queued when a cancellation is processed are dropped. Holding the lock across a blocking send was rejected because backpressure would then stall the stdin reader; writer acknowledgement was rejected because the spec handles that race client-side (a frame already written cannot be recalled, and clients ignore late responses).
- Route informational logging to stderr through one process-wide switch set at MCP startup, rather than threading a writer through every layer reachable from a tool call. Keep result and error envelopes in their own shapes: `resultType`, `ttlMs`, `cacheScope` and `serverInfo` where the schema defines them, no `_meta` on JSON-RPC errors, and no cache hints on tool results.
- Update the Quint models and MBT drivers in the same change, since the verification gate requires every spec to be referenced by a driver, and add a subprocess contract test for what unit tests cannot observe: the frames a real process writes to stdout, and the startup path that used to log there.

## References

- [MCP specification 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28)
- [Key changes in 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28/changelog) — statelessness, `server/discover`, required `resultType`
- [Pinned schema @ `84d8457`](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/84d845729393f79b3fd3d392aa17cb86f5b6f95c/schema/2026-07-28/schema.json) — the exact wire contract this server was checked against
