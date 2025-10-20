Project assets for packaging

Replace the placeholder files in this folder with real icons and resources before creating platform bundles.

Recommended files:
- icon-512.png — high-resolution PNG (512x512)
- icon-256.png — medium PNG (256x256)
- icon.png — fallback PNG (256 or 512)
- icon.icns — macOS ICNS bundle
- icon.ico — Windows ICO file with multiple sizes
- precommit-rs.desktop — Linux desktop file (already present)

When you have these in place, cargo-bundle and the GitHub Actions workflow will include them in generated packages.
