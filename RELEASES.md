Release automation notes

- The GitHub Actions workflow `.github/workflows/bundle.yml` builds artifacts on Linux, macOS, and Windows.
- After building it creates a release using `softprops/action-gh-release` and attaches any produced bundles.

Signing and notarization

- macOS signing: store a P12 certificate in `secrets.MAC_SIGNING_P12` and its password in `secrets.MAC_SIGNING_PASSWORD`. You will need to add steps to import the certificate and run `codesign` and `notarytool` or `altool` to notarize.
- Windows signing: add your code-signing tools and secrets (not included in this workflow).

Release body

- If the hooks produce a `PRECOMMIT_CHANGELOG.md` file that is uploaded as an artifact by the build job, the release body will include it. Otherwise a default "Auto-built release" message is used.

Artifacts

- The workflow uploads artifacts named `precommit-rs-bundles-<runner.os>` for each matrix row; the release job downloads them and attaches to the new release.

Linux (local)

- Install `appimagetool` (or let the workflow use the downloaded AppImage) and then run:

```bash
cargo build --release
cargo bundle --release
# If cargo-bundle created a linux AppDir under target/release/bundle/linux you can create an AppImage with appimagetool:
appimagetool target/release/bundle/linux
```

Windows (local)

- Install NSIS (makensis) and then run:

```powershell
cargo build --release
cargo bundle --release
# If cargo-bundle produces an NSIS script/bundle you can run makensis on the generated installer script
makensis path\to\installer.nsi
```
