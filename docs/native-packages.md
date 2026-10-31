# Windows installer and Arch package

Native packages wrap the existing portable application binaries. For this preview,
application version **0.3.0** is pinned by tag **v0.3.0**; adding native packaging
does not move that tag or rebuild/replace the application. The native packaging
recipes have their own source revision, recorded in the packaging-source bundle.
The portable ZIP/tarball remain available alongside the native packages.

## Windows x64

Download **`fm-editor-0.3.0-windows-x64-setup.exe`** from the release assets. Verify it
against the release's `SHA256SUMS`, then run it.

- Installs for the current user under `%LOCALAPPDATA%\Programs\FM Editor` by
  default. No administrator prompt is required for installation.
- Adds a Start Menu shortcut and an uninstall entry. The desktop shortcut is
  optional and unchecked by default.
- The installer is unsigned; Windows may show an unknown-publisher warning.
- Start FM with a save loaded, then launch FM Editor from the Start Menu. If FM
  itself is elevated, matching privileges may be needed for memory access; the
  installer does not elevate the app or alter the game's permissions.
- Uninstall through Windows Settings → Apps. Preferences in
  `%APPDATA%\fm-editor` are retained. The installer never launches the app as an
  installation action or removes external user settings.

OpenGL 2.0+ graphics support is still required. CI's software-rendering DLLs are
only test tools and are not bundled into the installer.

## Arch Linux / CachyOS x86-64

Download **`fm-editor-bin-0.3.0-1-x86_64.pkg.tar.zst`**, verify its checksum, then run:

```bash
sudo pacman -U ./fm-editor-bin-0.3.0-1-x86_64.pkg.tar.zst
fm-editor
```

The package provides a menu entry/icon, `/usr/bin/fm-editor`, documentation under
`/usr/share/doc/fm-editor-bin`, and its GPL licence under
`/usr/share/licenses/fm-editor-bin`. It is called **fm-editor-bin** because it uses
the same prebuilt Linux executable as the portable download. It declares its
runtime dependencies, provides `fm-editor`, and conflicts with an alternative
package of that name.

File dialogs use XDG desktop portals; install an appropriate backend for your
desktop if it is not already present. This package is unsigned and is not yet
published to the AUR or an Arch repository. If your local package policy requires
signatures, review/sign/trust it explicitly rather than disabling signature checks
globally.

Remove the package with:

```bash
sudo pacman -R fm-editor-bin
```

User preferences are not removed. The package has **no installation hooks that
change capabilities, ptrace settings or game permissions**. If attachment fails
with a memory-access error, review the
[Linux setup instructions](https://github.com/barak14/fm-editor/blob/v0.3.0/README.md#linux--proton)
before making security changes. For the native package, the executable path is
`/usr/bin/fm-editor`, not a `target/release` path. Do not run the whole GUI as root
as a default workaround.

## Building or reviewing the packages

The **Native packages** workflow uses the pinned, successful Actions build for the
existing application tag. `packaging/releases/<tag>.json` (for example `v0.3.0.json`) records the source commit,
run, original archive hashes and executable hashes. CI verifies them before making
native packages. It remains read-only: GitHub hides private draft releases from
read-only tokens, so this does not fetch draft assets or grant write permissions.
It never publishes a release or moves a tag.

Containers are normalized into deterministic native inputs (plain USTAR for Linux,
stored ZIP for Windows), with their own pinned checksums. Executable bytes remain
identical to the tagged preview's portable binaries; only archive layout and the
generated source-archive filename reference are normalized.

- Windows compilation uses pinned, SHA-256-verified **Inno Setup 6.7.4**. The Python
  wrapper extracts a verified private snapshot and supplies paths/version to the
  `.iss` recipe without shell interpolation.
- Arch uses an official digest-pinned Arch base-devel container. `makepkg` runs
  as a non-root builder. System dependency installation and package install/remove
  tests happen only inside that ephemeral container, not on a user's machine.
- `packaging/arch/PKGBUILD` pins the local Linux input tar, desktop file and SVG by
  SHA-256. The input tar is bundled with packaging source. Updating a release
  requires updating pins deliberately; there is no `SKIP` or automatic rewrite.
- The workflow is configured to check install/uninstall, application identity,
  shortcuts/desktop resources, disconnected GUI startup, and preservation of
  settings. These checks do not replace the user's live FM attach/edit/Undo test.

The additional **`fm-editor-0.3.0-packaging-source.tar.gz`** contains the native
packaging recipes, icons, helper scripts, tests, workflow, licence and this guide,
plus verified binary inputs under `inputs/`. This is a binary-package recipe bundle,
not a substitute for the separate application source archive.
`PACKAGING-INFO.json` identifies application and packaging revisions. The application
source archive and tag remain unchanged.

Initial CI bootstrap uses pinned Actions artifacts, which expire after 14 days.
The source bundle retains the verified inputs for offline rebuilds afterward;
missing/expired CI inputs fail closed rather than silently using another build.

For local builds, extract that bundle and use Python 3.10+:

```bash
python3 Tools/stage-arch-package.py \
  --archive inputs/fm-editor-0.3.0-linux-input.tar --checksums inputs/SHA256SUMS \
  --commit <full-application-commit> --version 0.3.0 --output-dir dist/arch
cd dist/arch
makepkg --syncdeps
```

Run `makepkg` as your normal user. It validates the pinned local input and needs no
private-release download. The recipe is not submitted to the AUR; before any AUR
submission, choose a durable public upstream source URL and follow AUR guidelines.

On Windows, with ISCC installed:

```powershell
python Tools/package-windows.py --archive inputs/fm-editor-0.3.0-windows-input.zip --checksums inputs/SHA256SUMS --commit <full-application-commit> --version 0.3.0 --output-dir dist/native-windows --iscc 'C:\Path\To\ISCC.exe'
```

Use the full application commit from the release's portable `BUILD-INFO.json` or
resolved tag, not the packaging helper's checkout revision. Source/checksum checks
fail closed on mismatches and output paths are never silently overwritten.
