# Releasing Belgr

Releases are maintainer-driven. This is the tagging runbook; see
[CONTRIBUTING.md](CONTRIBUTING.md) for development setup, runtime invariants,
tests, and dependency-license maintenance.

## Versions

The release version is set once, in `[workspace.package]` in the root
`Cargo.toml`; every workspace crate inherits it via `version.workspace = true`,
so they cannot drift apart. After changing that one value, run
`node scripts/release-version.mjs sync` to project it into the internal
dependency requirements under `[workspace.dependencies]`, then run
`cargo update --workspace` to refresh the workspace entries in `Cargo.lock`.
CI runs the script's `check` mode so generated dependency versions cannot
drift. Member manifests inherit the dependencies and contain no release
versions. `install.sh`'s `SCRIPT_VERSION` is an independent installer logging
revision and is not automatically synchronized to product releases.

`licenses/THIRD_PARTY_LICENSES.html` embeds the workspace crate versions, so a
version bump must regenerate it. CI diffs the checked-in report against a fresh
`cargo about generate` and fails on any difference.

## What a tag triggers

A `vX.Y.Z` tag triggers the GitHub release and docs workflows.

The release workflow opens with a coverage gate and builds nothing until it
passes. CI's branch and pull request triggers do not match tags, so this is the
only check that re-runs against the tagged tree. Collecting coverage runs the
whole workspace test suite, which means a failing test and a coverage
regression both stop the release; tagging a commit whose coverage run was red
on master fails here rather than shipping.

The gate covers Linux tests and the coverage baseline only. Formatting, Clippy,
the macOS and Windows test runs, the Android target check, and the
dependency-license checks stay pull request checks, so a tag still relies on the
tagged commit having passed CI on master.

The builds cover Linux x86-64 and ARM64, Android ARM64, Windows x86-64, and a
universal macOS archive. Desktop archives contain `mj` and the voice worker;
Android omits the voice worker. Every archive includes the applicable licenses
and notices and is shipped with a SHA-256 sidecar.

## Distribution

Belgr releases only through GitHub Releases. A successful tagged build attaches
the platform archives and their SHA-256 sidecars to the generated release; no
crates.io, npm, PyPI, Homebrew, or other package-registry publishing runs.

The `uvx brokk acp` command used to launch Anvil is a runtime dependency of
Belgr, not a Belgr distribution channel.

## Discord announcement

To announce a published GitHub Release in Discord, set the
`DISCORD_RELEASE_WEBHOOK_URL` repository Actions secret to the target channel's
webhook URL. The release workflow reuses GitHub's generated release notes,
prevents mentions from being parsed, suppresses automatic link embeds, and
leaves a failed Discord delivery as a warning so it cannot invalidate an
already-published release.

## Before tagging

Confirm that:

1. Every workspace crate manifest and its `Cargo.lock` workspace entry matches
   the intended tag.
2. Formatting, Clippy, release builds, tests, and relevant cross-platform or
   packaging checks pass.
3. Dependency-license policy and generated notice reports are current.
4. User-facing installation, configuration, and release documentation reflects
   the shipped behavior.
5. The release commit is merged and the tagged commit is the exact commit meant
   to be released.
