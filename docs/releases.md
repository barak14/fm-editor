# Test builds and release preparation

The **Build and package** GitHub Actions workflow produces downloadable test-build
artifacts. It does **not** create tags, publish GitHub Releases, sign binaries, or
change game/system permissions. Its token has read-only repository permissions.

## Download a test build

1. Open [Actions](https://github.com/barak14/fm-editor/actions), choose **Build and
   package**, and select a successful run for the intended branch and commit.
2. Download `fm-editor-x86_64-pc-windows-msvc` for Windows or
   `fm-editor-x86_64-unknown-linux-gnu` for Linux. GitHub requires a signed-in account
   to download Actions artifacts, which expire after 14 days.
3. Extract the downloaded artifact wrapper. It contains a Windows ZIP or Linux
   tar.gz, a matching source tar.gz, and `SHA256SUMS`. Verify the checksums, then
   extract the platform archive and run `fm-editor.exe` or `./fm-editor` inside it.

The separate **Native packages** workflow wraps an existing versioned release's
verified portable assets. Its artifacts are `fm-editor-windows-installer`,
`fm-editor-arch-package` and `native-packaging-source`. See
[native installation/build instructions](native-packages.md). It neither moves
the application tag nor publishes a release; install/remove tests run only in
hosted runners or ephemeral containers, not on a user's machine.

For Linux, run `sha256sum -c SHA256SUMS` from the artifact's extracted directory.
On Windows, compare `Get-FileHash <archive> -Algorithm SHA256` with the corresponding
line in `SHA256SUMS`. Checksums detect corruption, not a malicious build; trust the
repository and inspect the run's source commit. Do not use artifacts from an
unreviewed pull request as if they were an approved release.

Each platform archive has a single version/target/commit-named folder containing:

- The **release editor only**, not the diagnostic binaries or debug editor.
- README, GPL licence, changelog, shared project guidance and roadmap.
- Tracked documentation, including the README screenshots.
- `BUILD-INFO.json` identifying version, target, commit and release profile.
- `SOURCE.txt` pointing to the exact commit and matching source archive.

Local settings, credentials, `.claude/`, `.rusty/`, memory snapshots and build caches
are not included. Source is created from the exact committed tree using `git archive`
with line-ending conversion disabled; packaging refuses a dirty tracked checkout.
The source archive identifies the
application source; dependency notices/source obligations still need review before
public release distribution (see the release checklist below).

## What CI verifies

On pushes, pull requests and manual dispatch, separate native jobs run on
**Windows Server 2022 x86-64** and **Ubuntu 22.04 x86-64**:

1. Install stable Rust and Linux native development libraries where needed.
2. Run the Python packaging/startup-gate tests.
3. Run locked `cargo check --all-targets`, `cargo test`, and the release build for
   the job's explicit target. The lockfile is committed and must not change.
4. On Linux, check `ldd` output for unresolved libraries.
5. Start the release GUI, require it to survive three seconds, and stop it. The
   Linux job uses Xvfb/software rendering; both jobs reject a debug MCP listener on
   the default port. This is a **disconnected startup smoke test**, not live FM
   attachment, rendering-accuracy or edit/Undo verification.
6. Create the platform/source archives and checksums, then upload the artifacts.

The Windows hosted runner has no suitable hardware OpenGL context. Its smoke test
uses pinned, SHA-256-verified [Mesa 26.2.4 software OpenGL](https://github.com/pal1000/mesa-dist-win/releases/tag/26.2.4)
with a copy of the release executable in the runner's temporary directory. The
software-rendering DLLs are **not** copied into the build output or shipped in the
artifact. The test still runs the real release GUI; it does not bypass the startup
gate or change the application's renderer.

The Windows job requests a statically linked C runtime. The executable remains
unsigned, so Windows may warn about an unknown publisher. Signing remains future
work. The optional installer is built by the separate Native packages workflow.

The Linux artifact uses an Ubuntu 22.04 baseline (glibc 2.35); it is not a fully
static or universal Linux package. It needs a desktop session and the system's
X11/Wayland/OpenGL libraries and drivers, including `libxkbcommon-x11.so.0`
(`libxkbcommon-x11-0` on Ubuntu) for the X11 backend. Check `ldd ./fm-editor`
and startup logs after extraction: dynamically loaded libraries do not all appear
in `ldd` output. See [Linux/Proton setup](../README.md#linux--proton) for FM process
memory permissions; the archive does not grant capabilities or relax ptrace rules.

Actions are pinned by commit. Rust stable and runner image updates can still change
build output; the exact version/target/source is recorded, but this is not a promise
of bit-for-bit reproducible builds.

## Run locally

Use a clean committed checkout and Python 3.10 or newer:

```bash
python3 -m unittest discover -s Tools/tests -p 'test_*.py' -v
cargo check --locked --all-targets
cargo test --locked
cargo build --release --locked --bin fm-editor --target x86_64-unknown-linux-gnu
python3 Tools/package-release.py --target x86_64-unknown-linux-gnu --output-dir dist
```

On native Windows use `python`, target `x86_64-pc-windows-msvc`, and set
`RUSTFLAGS=-C target-feature=+crt-static` as CI does. Do not package a cross-compiled
binary and call it Windows-tested without running the Windows job.

`Tools/smoke-release.py --binary <release-executable>` can run in an isolated desktop
with port 7799 unused. On Linux use `xvfb-run -a python3 Tools/smoke-release.py ...`
or a private Wayland session. It starts/stops the app; do not point it at a debug
binary or use it to interrupt an editor someone is testing.

`dist/` is ignored. Packaging refuses to overwrite existing output files: remove
only the known previous build outputs or choose a fresh output directory.

A branch push triggers the workflow immediately once the workflow file is pushed.
The Actions **Run workflow** button/manual dispatch requires the workflow to exist
on the repository's default branch first. No push or dispatch publishes a release.

## Before a public prerelease

- Choose the source commit, including any UI work still in a worktree. Commit and
  push it, and require both native jobs to pass for that exact revision.
- Choose a version/tag matching `Cargo.toml`; prepare versioned changelog notes.
- Extract and launch both real packages on target machines. Verify attach, one
  controlled edit and Undo against the stated FM builds; restore test changes.
- Review third-party licence notices and provide the required corresponding source
  (including dependency/build material where required), not just a binary ZIP.
- State compatibility limits and untested builds honestly, particularly Game Pass.
- Create a **draft/prerelease** with the reviewed archives, checksums and source;
  publish only after review. This publishing workflow has not been implemented.
- When adding native packaging to an already tagged application, keep the original
  portable assets, source and tag unchanged. Attach the installer, Arch package and
  matching packaging-source bundle; record the independent packaging revision and
  regenerate the combined checksums. Do not silently replace the tagged binary.

No installer, automatic updater, signing identity, release secret or write-enabled
GitHub token is needed to try these initial artifacts.
