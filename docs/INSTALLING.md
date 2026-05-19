# Installing Promptibrary

Promptibrary V1 ships **unsigned by Apple's authority** for personal-use builds. The first time you open the `.app` on macOS, Gatekeeper will refuse to launch it with the dialog *"Promptibrary cannot be opened because the developer cannot be verified."*

This is expected. Two ways past it, both one-time per install.

## Option 1 — right-click → Open (recommended)

1. In Finder, locate `Promptibrary.app` (typically in `~/Applications` or `~/Downloads`).
2. **Right-click** (or Control-click) the app icon → **Open**.
3. macOS shows a milder dialog: *"macOS cannot verify the developer of 'Promptibrary'. Are you sure you want to open it?"* Click **Open**.
4. macOS remembers the exemption. Subsequent launches happen normally.

## Option 2 — strip the quarantine attribute

If you prefer the command line, or if Option 1 doesn't work (rare on managed devices):

```bash
xattr -dr com.apple.quarantine /Applications/Promptibrary.app
```

Adjust the path if you installed Promptibrary somewhere other than `/Applications`. This removes the `com.apple.quarantine` xattr macOS uses to gate first-launch verification.

## Updates after first launch

Once the initial launch is past Gatekeeper, the in-app updater (`tauri-plugin-updater`) handles subsequent updates over HTTP. Updates downloaded programmatically don't carry the `com.apple.quarantine` xattr, so they install silently — you won't need to repeat the right-click / xattr dance for every release.

Each update bundle is signed by the Promptibrary release pipeline using a **Tauri signer keypair** distinct from Apple code signing. The app refuses to install any update whose signature does not verify against the embedded public key. There is no trust-on-first-use fallback.

## Why isn't Promptibrary signed by Apple?

V1's posture is personal-use: the build pipeline doesn't pay for an Apple Developer Program enrollment ($99/year) or notarize bundles through `xcrun notarytool`. Apple code signing is tracked as a deliberate V2 candidate (see `docs/V2-CANDIDATES.md`) if and when Promptibrary becomes a distributed application.

The Tauri-signer signature on every update bundle is **mandatory and verified** — it just isn't the same thing as Apple's Gatekeeper authority. Two different signatures, two different purposes.

## Linux

The `.AppImage` and `.deb` artifacts produced by the release workflow are unsigned. There is no Gatekeeper equivalent on most Linux distros; install the bundle the usual way for your distribution:

```bash
# AppImage
chmod +x Promptibrary_*_amd64.AppImage
./Promptibrary_*_amd64.AppImage

# Debian / Ubuntu
sudo dpkg -i promptibrary_*_amd64.deb
```

The in-app updater behaves identically on Linux.
