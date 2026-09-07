# Belgr

Belgr (`belgr`) is a terminal client for **[Draupnir](https://github.com/foundev/draupnir)**,
our fork of Brokk's portable [Anvil](https://github.com/BrokkAi/anvil)
[ACP](https://agentclientprotocol.com/get-started/introduction) coding agent.
It is a fork of [Mjolnir](https://github.com/BrokkAi/mjolnir): Draupnir and
Anvil both register as platform ACP routes at startup, and every agent from
the official [ACP registry](https://agentclientprotocol.com) is available as
an opt-in ACP server alongside the built-in Codex and Claude routes.

In the myth, Brokk worked the bellows (*belgr*) while Eitri forged Mjolnir. Belgr keeps
Draupnir's fire going.

## What it keeps from Mjolnir

- The full terminal workflow: sessions, worktree sessions, parallel subagents,
  mid-turn steering, integrated review, and the remote-control surface.
- Shared project knowledge (`belgr memory ...`, `/memory`).
- Local voice input on macOS, Linux, and Windows.
- The web viewer and `belgr server` remote workflow.

## What is different

- **Platform routes plus the ACP registry.** Draupnir registers as the implicit
  platform team at startup on every target (Mjolnir did this only on Android);
  Anvil takes over when it is the only one installed or when Draupnir is
  disabled in `/mjconfig`. The Codex and Claude built-ins are back in the
  inventory, and every agent from the ACP registry appears as an opt-in ACP
  server — enable one under `/mjconfig` to add it to model discovery.
  Binary-distributed registry agents install on first use.
- **Separate install identity.** Binary `belgr`, config in `~/.config/belgr/`, project
  marker dir `.belgr/` — a Belgr install never collides with a Mjolnir install on the
  same machine.
- **Distribution**: versioned binary archives are released only through GitHub.

Draupnir launches through the `draupnir` binary on `PATH`; set `MJ_DRAUPNIR_PATH`
to use a specific local Draupnir binary instead. Anvil follows the same pattern
with the `anvil` binary and `MJ_ANVIL_PATH`.

## Build

```bash
cargo build --release
./target/release/belgr
```

The default desktop build needs the platform WebView dev packages (see
`AGENTS.md`). `cargo test` and `cargo clippy --all-targets -- -D warnings` gate
changes.

## Status

Experimental. Whether this stays under foundev or moves into Brokk proper is TBD.
The `docs/` site is still the Mjolnir documentation and has not been rebranded.

License: GPL-3.0-only, same as Mjolnir.
