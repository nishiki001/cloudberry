# Packaging

Everything the release needs lives here; `.github/workflows/release.yml` builds the formats with
`scripts/package.sh` when a `v*` tag is pushed. App id everywhere: `io.github.nishiki001.Cloudberry`.

| Path | What |
|---|---|
| `io.github.nishiki001.Cloudberry.desktop`, `….metainfo.xml` | launcher entry and AppStream data (validated with `desktop-file-validate`, `appstreamcli validate --pedantic`) |
| `flatpak/` | Flatpak manifest + `cargo-sources.json` (regenerate after Cargo.lock changes: `pipx run scripts/flatpak-cargo-generator.py Cargo.lock -o packaging/flatpak/cargo-sources.json`); `scripts/flathub-manifest.sh <tag> <commit>` prints the Flathub flavour |
| `rpm/cloudberry.spec`, `../.packit.yaml` | Fedora COPR build from the release tarball + `cloudberry-vendor.tar.xz` |
| `aur/cloudberry-bin`, `aur/cloudberry` | Arch PKGBUILDs (+ `.SRCINFO`); `scripts/aur-update.sh <version>` after a release |
| `homebrew/cloudberry.rb` | cask for the `nishiki001/homebrew-tap` repo (the release workflow bumps it when `HOMEBREW_TAP_TOKEN` exists) |
| `macos/` | `Info.plist`, optional Developer ID signing + notarization (`sign-notarize.sh`, runs when the Apple secrets exist) |
| `windows/cloudberry.nsi` | NSIS installer |
| `winget/`, `scoop/` | manifests (optional) |

Flatpak and `--features system-tools`: the Flatpak bundles yt-dlp and deno (pinned, checksummed) and
never downloads or updates executables at run time. Distro packages may do the same
(`cargo build --release --features system-tools`) and depend on the distro's yt-dlp and deno.

## The owner's steps
See the maintainer's release checklist (kept outside the repository).
