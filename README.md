# Belgr

Belgr (`belgr`) is a terminal client for **[Anvil](https://github.com/BrokkAi/anvil)**,
Brokk's portable [ACP](https://agentclientprotocol.com/get-started/introduction) coding
agent. It is a fork of [Mjolnir](https://github.com/BrokkAi/mjolnir) with Anvil as the
only ACP route on every platform — the Codex and Claude adapter plumbing is inert and
being removed.

In the myth, Brokk worked the bellows (*belgr*) while Eitri forged Mjolnir. Belgr keeps
Anvil's fire going.

## What it keeps from Mjolnir

- The full terminal workflow: sessions, worktree sessions, parallel subagents,
  mid-turn steering, integrated review, and the remote-control surface.
- Shared project knowledge (`belgr memory ...`, `/memory`).
- Local voice input on macOS, Linux, and Windows.
- The web viewer and `belgr server` remote workflow.

## What is different

- **Anvil only.** Anvil registers as the implicit platform team at startup on every
  target (Mjolnir did this only on Android). There is no team selection; Codex and
  Claude routes never appear in the inventory.
- **Separate install identity.** Binary `belgr`, config in `~/.config/belgr/`, project
  marker dir `.belgr/` — a Belgr install never collides with a Mjolnir install on the
  same machine.
- **Distribution**: versioned binary archives are released only through GitHub.

Anvil launches through the `anvil` binary on `PATH`; set `MJ_ANVIL_PATH` to use a
specific local Anvil binary instead.

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
