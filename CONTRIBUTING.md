# Contributing to slot-flip

Thanks for wanting to help. This is a personal fork of
[slot](https://slot-cfw.fyi) by Brandon T. Kowalski, ported to the **Miyoo
Flip** and, experimentally, other Miyoo devices. Contributions of all sizes are
welcome — including "I tested it and here's the log", which is genuinely one of
the most useful things you can do right now.

By contributing you agree your work is licensed under the project's licence,
**GPL-3.0-or-later** (the same as upstream).

---

## The most useful things you can do

### 1. Test an experimental device build

I only own a Miyoo Flip, so the other ports depend on logs from people who own
the hardware. See
**[TESTERS.md](https://github.com/boomchu-123/slot-flip/blob/experimental/TESTERS.md)**
for the full runbook. Short version:

- **Miyoo A30 (`my282`)** — has a Mali GPU, so it uses the real renderer and has
  a real chance of working. Completely unverified.
- **Miyoo Mini Plus (`miyoomini`)** — no GPU; the graphics path is still being
  written, so this build only probes hardware and writes a log.

Grab a pre-release, run it with a `DEBUG` file present, and post
`Slot/slot.log` (plus `Slot/input-trace.log`). That's how the Flip port got
finished: run it, read the log, fix, repeat.

### 2. Report a bug

Use the **[bug report](.github/ISSUE_TEMPLATE/bug_report.yml)** template. It
asks for the things that make a bug actionable:

- Device and build (stable `v*` or an `exp-v*` pre-release).
- **`Slot/slot.log`** — reproduce with an empty file named `DEBUG` next to
  `launch.sh` in `Slot.pak`, then attach the log.
- What you expected vs. what happened.

A bug with no log is very hard to fix, especially on hardware I don't own.

### 3. Request or pick up a feature

- Check **[ROADMAP.md](ROADMAP.md)** first — it lists what's planned.
- Open a **[feature request](.github/ISSUE_TEMPLATE/feature_request.yml)**.
- Want to build one? Comment on the issue and go for it. Issues labelled
  **`good first issue`** and **`help wanted`** are the best starting points.

### 4. Improve docs

Docs fixes are always welcome and easy to review. The docs that matter most:

| File | What it covers |
| --- | --- |
| `README.md` | Overview, install, device support, troubleshooting |
| `INSTALL.md` | Step-by-step install |
| `PORTING.md` | Where device coupling lives; how to add a device |
| `TESTERS.md` | How to test experimental device builds |
| `ROADMAP.md` | Planned work |

---

## Branches

| Branch | Purpose | Releases |
| --- | --- | --- |
| **`flip`** (default) | Stable. The Miyoo Flip build that ships. | `v*` tags → normal GitHub releases |
| **`experimental`** | Device ports and unproven work (`my282`, `miyoomini`). | `exp-v*` tags → **pre-releases** |

- **Bug fixes / stable improvements** → target `flip`.
- **New device ports / experimental work** → target `experimental`.

Experimental changes stay out of `flip` until they're verified on hardware, so
the stable build never regresses for existing users.

---

## Building

The device builds are cross-compiled in Docker so they match CI (and the
`rust:1-bullseye` image has the right glibc for the handhelds). Rust **1.96**,
edition 2021.

### Miyoo Flip (aarch64, stable)

```sh
docker run --rm -v "$PWD":/src -w /src rust:1-bullseye \
  cargo build --release -p slot --no-default-features --features flip --target-dir target-flip
```

The binary lands in `target-flip/release/slot`.

### Experimental targets (32-bit armhf)

Both the A30 and the Mini Plus are 32-bit ARM (Cortex-A7), so they build for
`armv7-unknown-linux-gnueabihf` — **not** aarch64. The Mini Plus firmware uses
glibc 2.28, so its build must use the matching Buster sysroot:

```sh
docker run --rm -v "$PWD":/src -w /src rust:1-buster sh -c '
  sed -i -e "s|deb.debian.org/debian buster|archive.debian.org/debian buster|g" \
    -e "s|deb.debian.org/debian-security|archive.debian.org/debian-security|g" \
    -e "s|security.debian.org/debian-security|archive.debian.org/debian-security|g" \
    /etc/apt/sources.list
  apt-get -o Acquire::Check-Valid-Until=false update
  apt-get install -y gcc-arm-linux-gnueabihf libc6-dev-armhf-cross
  rustup target add armv7-unknown-linux-gnueabihf
  CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc \
    cargo build --release -p slot --no-default-features --features miyoomini \
    --target armv7-unknown-linux-gnueabihf --target-dir target-mini'
```

For the A30, use `rust:1-bullseye` and `--features my282`. `libc6-dev-armhf-cross`
is required for both — without it the link fails with `cannot find Scrt1.o`.

### Host build (for iterating on UI/logic)

```sh
cargo build -p slot          # the `host` feature is on by default
```

---

## Testing

```sh
cargo test --workspace
```

**Note:** on Linux, a few targets are macOS-gated and will fail to compile if
pulled in (the `flip_smoke` example, `slot-gfx/tests/pipeline.rs`, and the
`record` example). To run the meaningful suite without those:

```sh
cargo test -p slot --lib \
           -p slot-store --lib \
           -p slot-power --lib \
           -p slot-input --lib \
           -p slot-ui --lib \
           -p slot-retro --lib \
           -p slot-gfx --lib
```

The integration suites (`slot-store`, `slot-power`, `slot-ui`) are also worth
running when you touch shared code:

```sh
cargo test -p slot-store -p slot-power -p slot-ui --tests
```

Please keep the suite green. If you add a feature, a test is welcome but not
mandatory — a clear explanation in the PR is fine.

---

## Code style

- `cargo fmt` before committing; there's a `task fmt` too.
- `cargo clippy` where practical.
- Match the surrounding style. The codebase favours small, direct functions and
  comments that explain **why**, not **what**.
- Keep device-specific code behind a Cargo feature (`flip`, `my282`,
  `miyoomini`) so other targets don't regress. See `PORTING.md`.
- **Data-driven over hard-coded** where possible: button maps and settings
  belong in `Config/*.ini`, not in a rebuild.

## Commits & pull requests

- Small, focused commits with a clear message. Prefix with `type: summary`
  (`feat:`, `fix:`, `docs:`, `ci:`, `refactor:`) like the existing history.
- In a PR, say **which device(s)** you tested on, and **what you couldn't
  test**. "Untested on hardware" is fine and honest — that's what the
  experimental branch is for.
- For device ports, include the `slot.log` from a real run.

### Review policy

- `flip` and `experimental` are **protected**. Nothing lands directly; changes
  go through a pull request that passes CI (`pr-checks`) and gets reviewed.
- Open PRs from **your own fork** (`flip` for stable fixes, `experimental` for
  device work). You don't need push access to the repo.
- Only the maintainer has write access, so **branches in this repo are
  maintainer-only** — that's deliberate.
- **Automated dumps and unsolicited bot PRs are closed without review.** If
  you're an AI agent or a script, don't open a PR: open an issue with your
  reasoning instead, and let a person decide.
- First-time contributors' CI runs need manual approval before they execute —
  that's a security default, not a judgement on your patch.
- If a PR goes quiet for a long time, a polite bump is fine; PRs that are
  abandoned may be closed.

### Secrets and personal data

This repository is public and its history is permanent, so:

- **Never commit credentials, tokens, private keys, or personal data** (home
  paths, private IPs, email addresses, device serials, save files). Remove a
  secret and it is still in history: **rotate it first**.
- Run `sh tools/install-hooks.sh` once per clone. It enables the hooks in
  `.githooks/` and seeds a private denylist of your own names, email and paths
  in `.git/info/sensitive-patterns` (never committed). Add your phone number,
  an old email address or a device serial to that file.
- Pre-commit and pre-push refuse a finding, and the `secrets` workflow runs the
  same check plus [gitleaks](https://github.com/gitleaks/gitleaks) over the full
  history on every push and pull request.
- **Do not use `--no-verify`** (or `git commit -n`). It skips pre-commit and
  commit-msg, but `post-commit` still scans the new commit and, if it holds a
  finding, undoes the commit and leaves the changes staged. During a rebase or
  merge it cannot undo safely, so it only warns; CI is the backstop there.
- A deliberate example in a test may be marked with `check-sensitive: allow` on
  its line; do not use that marker to smuggle a real value.

---

## Assets & legal

- **Do not commit ROMs, BIOS files, or copyrighted cartridge art.** Game data
  is the user's own responsibility; only add code, docs, and clearly-licensed
  assets.
- The emulator cores are bundled with their licences in `System/licenses/`.
  Don't replace them without updating the licence files.
- `AI_DISCLOSURE.md` explains how this project is built. AI-assisted
  contributions are fine; be accurate about them.

---

## Working on the codebase

[AGENTS.md](AGENTS.md) is the developer guide for this fork: how it differs from
upstream, how to sync with upstream without losing fork behaviour, how to verify
changes on a real device, and how to write release notes. It is written for
humans and AI agents alike.

---

## Be decent

This is a hobby project. Be patient, be specific, and assume good faith.
Bug reports about a device I don't own are best treated as collaboration, not
complaints — the log you paste is the thank-you.
