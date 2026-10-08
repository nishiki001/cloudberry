# Open-source desktop client for YouTube Music (Rust + Slint)

Unofficial client, GPL-3.0, public on GitHub. Fast, native GUI styled as a classic desktop music player (UI follows docs/DESIGN.md v2).
Targets: Linux + macOS first. Windows: must compile, no extra effort.
Dev machine: Ultramarine Linux (Fedora, KDE Plasma, Wayland). Owner has YouTube Music Premium.
Crate/binary name = whatever `Cargo.toml` says. Never brand the app as "YouTube Music"; README says "unofficial client for YouTube Music, not affiliated with Google".

## Autonomous mode
- Owner is mostly away. Don't ask questions. Make the reasonable call, log it as one line in `docs/PLAN.md` → Decisions, continue.
- Run the WHOLE plan in one go: finish a milestone, then start the next. Stop only when everything is done or everything left is blocked.
- After any context compaction or restart: re-read `docs/PLAN.md` first, resume at the first unchecked item. PLAN.md is the only source of truth for progress.
- Read only the `docs/REFERENCE.md` / `docs/DESIGN.md` section you need right now.
- Loop per item: implement → `cargo fmt` → `cargo clippy -q --all-targets -- -D warnings` → `cargo test -q` → the item's **Verify** → tick it in PLAN.md → commit (`git add -A && git commit -m "<milestone>: <what>"`).
- Needs human eyes/ears beyond screenshots (audio, media keys, macOS)? Add to PLAN.md → Human checks. Don't wait.
- Same failure after 3 different attempts → PLAN.md → Blockers, take the documented fallback or move to an independent item.
- NEVER: `git push`, `cargo publish`, touch files outside the repo (except the app's own config/cache/data dirs), read browser profiles or cookie files directly, print cookie/token values.

## Model split
- This session (Sonnet) writes the code. Opus only reviews, via the `reviewer` subagent.
- After each milestone's Verify passes and is committed: delegate to `reviewer` with the milestone name and commit range.
- VERDICT: FIX → fix every MUST FIX, re-run checks, commit, re-review once. Still FIX → fix what you can, log the rest in Blockers, move on. Max 2 reviews per milestone.
- SHOULD FIX items: do them if each is < ~20 lines, else log as deferred in Decisions.
- Never invoke the reviewer for single steps, only whole milestones.

## Architecture rules (non-negotiable)
- Stream URLs are resolved only by yt-dlp (`yt-dlp -g`/`-J` with the cookie file). We download the resolved URL and decode it ourselves (native backend) or hand the watch URL to mpv (fallback backend). Never implement signature/n-challenge/PO-token/SABR logic. No ad- or paywall-bypass features.
- Playback: a `Player` trait in `src/player/` with two backends: native (symphonia + cpal, default; feeds the analyzer) and `libmpv2` (cargo feature `mpv`, fallback). If libmpv won't build/link: spawn `mpv --idle --input-ipc-server=<sock>`, JSON IPC behind the same trait.
- libmpv ignores the user's mpv.conf. Set every option in code (REFERENCE §mpv). The analyzer is driven only by real decoded samples, never faked.
- Browse/search/library: InnerTube (`WEB_REMIX`), modeled on Python `ytmusicapi`. Auth = cookies + SAPISIDHASH (REFERENCE §auth).
- Parse InnerTube JSON as `serde_json::Value` with a `nav(&v, &[..])` helper. Parsers never panic; skip unparseable items (debug log).
- GUI: Slint (`.slint` files in `ui/`, compiled by `slint-build`). Slint owns the main thread. A tokio runtime on a background thread runs API, player and image tasks; talk to the UI only via `slint::invoke_from_event_loop` / `Weak::upgrade_in_event_loop`. Never block the UI thread.
- Core logic (`src/core/`: queue, state, API, player) has no Slint imports and is unit-tested on its own.
- Every feature also has a headless subcommand (`search`, `play`, `auth`, `liked`, `playlist`, `radio`, `lyrics`, `doctor`) with `--json`, so it's testable without the GUI. No args = GUI.
- Screenshot mode: `--screenshot <out.png> [--theme light|dark] [--view <name>] [--fixture <name>]` renders a view with fake data offscreen (REFERENCE §screenshots) and exits. Use it to look at your own UI work (Read the PNG). Never require network for it.
- `src/deps.rs` startup check for mpv/libmpv, yt-dlp, deno. Also search `~/.local/bin`, `~/.deno/bin`, `/opt/homebrew/bin`, `/usr/local/bin`; prepend found dirs to PATH so mpv's yt-dlp sees deno. Missing → GUI: friendly dialog with per-OS install commands; CLI: same text, exit 1.
- Config: TOML in the platform config dir (`directories`). Cache (thumbnails) and data (cookies, logs) in their platform dirs. Logs via `tracing` to a file.
- Performance budget (lightweight is the whole point): window visible < 400 ms after launch, RSS < 150 MB while playing, ~0% CPU when paused (no redraw loop), < 6% CPU on a 2018 4-core laptop while the disc spins.

- Every interactive component (menu, button, slider, popup, tab, list row…) needs an interaction test in `src/gui/interaction_tests.rs` (Slint testing backend: pointer move/click, keys, elements found by accessible label). Screenshots prove looks, not that clicks work; popups are not even rendered offscreen. Give new interactive elements an `accessible-label`.
- The build must be warning-free, including Slint compiler warnings (deprecated properties etc.); check with `cargo build 2>&1 | grep -ci warning` → 0.

## Privacy & repo hygiene
- Cookies only sent to `*.youtube.com` / `*.google.com`. Thumbnail requests (`*.googleusercontent.com`, `*.ytimg.com`) get NO cookies. Redact `Cookie`/`Authorization` in logs and errors.
- Exported cookie file: data dir, mode 0600. Never in the repo.
- Fixtures: trim (< 50 KB each), scrub the owner's name, email, channel/account IDs, avatar URLs. Prefer unauthenticated captures.
- New deps and bundled assets (fonts, images): maintained, GPL-3.0-compatible (OFL fonts are fine). One line in Decisions with license. Only original artwork: no logos or branding of YouTube, Google, or any existing player.

## Token economy (strict)
- Iterate with `cargo check -q --message-format=short 2>&1 | head -n 40`. Pipe long output through `head`/`tail`/`grep`.
- Never read `target/`, `Cargo.lock`, `LICENSE`, whole fixtures (use `jq` paths), font/binary files.
- Find with Grep/Glob, then Read with offset/limit. Files < 300 lines (`.slint` files too: one component per file).
- `cargo add` for deps. Learn a crate's API by grepping `~/.cargo/registry/src/*/<crate>-*/` before fetching docs.
- ytmusicapi research: fetch one raw file at a time (paths in REFERENCE), take only what you need, append a compact note to REFERENCE §Findings so no session fetches it twice.
- Screenshots: render at 1x, view one at a time, max 3 visual iterations per component before moving on (log leftovers in Human checks).
- Network/playback commands: `timeout 30 cargo run -q -- ...`. Automated playback tests use `APP_AO=null` (mpv `ao=null`).
- Replies: ≤ 5 lines per finished item. No recaps of code just written.
