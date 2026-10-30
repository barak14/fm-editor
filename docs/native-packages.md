# Windows installer and Arch package

Native packages wrap the existing portable application binaries. For this preview,
application version **0.1.0** is pinned by tag **v0.1.0**; adding native packaging
does not move that tag or rebuild/replace the application. The native packaging
recipes have their own source revision, recorded in the packaging-source bundle.
The portable ZIP/tarball remain available alongside the native packages.

## Windows x64

Download **`fm-editor-0.1.0-windows-x64-setup.exe`** from the release assets. Verify it
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

Download **`fm-editor-bin-0.1.0-1-x86_64.pkg.tar.zst`**, verify its checksum, then run:

```bash
sudo pacman -U ./fm-editor-bin-0.1.0-1-x86_64.pkg.tar.zst
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
[Linux setup instructions](https://github.com/barak14/fm-editor/blob/v0.1.0/README.md#linux--proton)
before making security changes. For the native package, the executable path is
`/usr/bin/fm-editor`, not a `target/release` path. Do not run the whole GUI as root
as a default workaround.

## Building or reviewing the packages

The **Native packages** workflow downloads the existing release assets and verifies
checksums, application version, full source commit, release profile and executable
format before creating native packages. It has read-only GitHub permissions and
never publishes a release or moves a tag.

- Windows compilation uses pinned, SHA-256-verified **Inno Setup 6.7.4**. The Python
  wrapper extracts a verified private snapshot and supplies paths/version to the
  `.iss` recipe without shell interpolation.
- Arch uses an official digest-pinned Arch base-devel container. `makepkg` runs
  as a non-root builder. System dependency installation and package install/remove
  tests happen only inside that ephemeral container, not on a user's machine.
- `packaging/arch/PKGBUILD` pins the portable archive, desktop file and SVG by
  SHA-256. Updating a release requires updating those pins deliberately; there is
  no `SKIP` checksum or automatic rewrite.
- The workflow is configured to check install/uninstall, application identity,
  shortcuts/desktop resources, disconnected GUI startup, and preservation of
  settings. These checks do not replace the user's live FM attach/edit/Undo test.

The additional **`fm-editor-0.1.0-packaging-source.tar.gz`** contains the native
packaging recipes, icons, helper scripts, tests, workflow, licence and this guide.
`PACKAGING-INFO.json` identifies both the application source and the independent
packaging revision. The existing application source archive remains unchanged.

For local builds, use Python 3.10+ and the portable archive plus `SHA256SUMS`:

```bash
python3 Tools/stage-arch-package.py \
  --archive fm-editor-0.1.0-linux-x64.tar.gz --checksums SHA256SUMS \
  --commit <full-application-commit> --version 0.1.0 --output-dir dist/arch
cd dist/arch
makepkg --syncdeps
```

Run `makepkg` as your normal user. The staged archive allows building while the
GitHub release is still a private draft; makepkg validates the pinned local bytes
rather than trying to download an unpublished URL.

On Windows, with ISCC installed:

```powershell
python Tools/package-windows.py --archive fm-editor-0.1.0-windows-x64.zip --checksums SHA256SUMS --commit <full-application-commit> --version 0.1.0 --output-dir dist/native-windows --iscc 'C:\Path\To\ISCC.exe'
```

Use the full application commit from the release's portable `BUILD-INFO.json` or
resolved tag, not the packaging helper's checkout revision. Source/checksum checks
fail closed on mismatches and output paths are never silently overwritten.
