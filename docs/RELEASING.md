# Releasing Promptibrary

This document describes the V1 release process for future-you. The release flow is fully automated via `.github/workflows/release.yml`; this doc explains the manual steps that bracket the automation and the operator-visible artifacts.

## Pre-flight (once per release)

1. **Decide the version.** SemVer per CHANGELOG. Bump in three places — they must agree:
   - `package.json` `version`
   - `src-tauri/Cargo.toml` `[package].version`
   - `src-tauri/tauri.conf.json` `version`
2. **Update `CHANGELOG.md`** with the new version's header, date, and the bullets covering user-visible changes.
3. **Run the full test suite locally:**
   ```bash
   pnpm typecheck && pnpm lint && pnpm test && (cd src-tauri && cargo test --lib)
   pnpm exec playwright test  # E2E
   pnpm test:visual           # macOS-only baselines
   ```
4. **Confirm the Tauri signer keypair is configured.** See *Signing keys* below. Without `TAURI_SIGNING_PRIVATE_KEY` in the GitHub Actions secrets the release workflow will fail at the `pnpm tauri build` step.
5. **Tag and push.** Tags `v*` trigger the release workflow.
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

## First dry-run (v0.0.1-rc1) — do this once before the real v0.1.0 cut

The L5 stop-condition requires verifying the release workflow end-to-end against a real `v*` tag push *before* the production release. The pre-release tag (`-rc*` / `-pre*`) ships as a draft release marked prerelease so it doesn't promote as `latest`.

**One-time prerequisites:**

1. Tauri signer keypair generated and pasted into `src-tauri/tauri.conf.json` (see *Signing keys* below).
2. `TAURI_SIGNING_PRIVATE_KEY` (and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if your key is passphrase-protected) added to GitHub Actions secrets.
3. `src-tauri/tauri.conf.json::plugins.updater.pubkey` is non-empty.

**Dry-run steps:**

1. Push the dry-run tag:
   ```bash
   git tag v0.0.1-rc1
   git push origin v0.0.1-rc1
   ```
2. Watch the workflow run: `gh run watch` or open the Actions tab. It should produce a draft release with `prerelease: true`.
3. Verify the artifact set on the draft release page — every entry below should be present:
   - `Promptibrary_0.0.1-rc1_aarch64.dmg`
   - `Promptibrary_0.0.1-rc1_x64.dmg`
   - `Promptibrary_0.0.1-rc1_aarch64.app.tar.gz` + `.sig`
   - `Promptibrary_0.0.1-rc1_x64.app.tar.gz` + `.sig`
   - `Promptibrary_0.0.1-rc1_amd64.AppImage.tar.gz` + `.sig`
   - `Promptibrary_0.0.1-rc1_amd64.deb`
   - `latest.json`
4. Verify `latest.json` resolves and contains valid signatures:
   ```bash
   curl -L "https://github.com/scalinity/Promptibrary/releases/download/v0.0.1-rc1/latest.json" \
     | jq '{version, platforms: (.platforms | keys), signatures: (.platforms | to_entries | map({(.key): (.value.signature | length)}) | add)}'
   ```
   The `signatures` map should report a positive length for each platform key.
5. Install the dry-run `.dmg` on a clean Mac to confirm the first-launch Gatekeeper bypass documented in `docs/INSTALLING.md` works.
6. Test the in-app updater path from a hypothetical "previous version" (just install rc1, confirm Settings → Check for updates resolves the manifest without errors).
7. **Tear down before v0.1.0:** delete the draft release page and the `v0.0.1-rc1` tag locally and remotely:
   ```bash
   gh release delete v0.0.1-rc1 --cleanup-tag --yes
   git tag -d v0.0.1-rc1
   ```

Only after this checklist is green should you cut the real `v0.1.0` per the Pre-flight section above.

## What the release workflow does

`.github/workflows/release.yml` fires on `v*` tag pushes:

1. **Matrix build** across `macos-14` (Apple Silicon, builds universal2) and `ubuntu-24.04`.
2. `pnpm install --frozen-lockfile`, then `pnpm tauri build` with the Tauri signing private key in the `TAURI_SIGNING_PRIVATE_KEY` env var (plus `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if your key is passphrase-protected).
3. **No Apple code signing.** No `APPLE_CERTIFICATE`, no `xcrun notarytool` invocation, no notarization step. The macOS bundle is unsigned by Apple's authority — this is the V1 personal-use posture. See `docs/INSTALLING.md` for the first-launch Gatekeeper bypass that ships to end users.
4. Generate `latest.json` (the updater manifest) with asset URLs and Tauri-signer signatures for every bundle in this release.
5. Upload artifacts to the GitHub Release:
   - `Promptibrary_<version>_aarch64.dmg`, `Promptibrary_<version>_x64.dmg`
   - `Promptibrary_<version>_aarch64.app.tar.gz` + `.sig` (Tauri signer)
   - `Promptibrary_<version>_x64.app.tar.gz` + `.sig`
   - `Promptibrary_<version>_amd64.AppImage.tar.gz` + `.sig`
   - `Promptibrary_<version>_amd64.deb`
   - `latest.json`

The visual regression suite runs on the `macos-14` matrix entry as a release gate: a tagged build cannot ship if any visual diff exceeds the threshold.

## Post-flight verification

1. **Check the GitHub Release page** — every artifact above is present.
2. **Verify `latest.json` resolves correctly.** From a clean machine: `curl -L https://github.com/<owner>/promptibrary/releases/latest/download/latest.json` and confirm the `platforms.darwin-aarch64.url` / `signature` fields point at this release's artifacts.
3. **Test the in-app updater path** by running a previous version of Promptibrary and triggering "Check for updates" in Settings. The updater should download, verify the signature, and prompt to restart.
4. **First-install Gatekeeper test (macOS).** On a clean Mac that has never seen Promptibrary, install the new `.dmg`, then open the `.app`. Gatekeeper should warn with the unverified-developer dialog. Use the right-click → Open path documented in `docs/INSTALLING.md` and confirm Promptibrary launches.

## Signing keys

Promptibrary signs every update bundle with a **Tauri signer keypair**, distinct from Apple's code-signing system. The app refuses to install any update whose signature does not verify against the embedded public key. There is no trust-on-first-use fallback.

### Generate the keypair (once, per repo)

```bash
# 1. Install the Tauri CLI if you don't have it already.
cargo install tauri-cli

# 2. Generate the keypair. Writes ~/.tauri/promptibrary.key (private)
#    and prints the public key to stdout.
mkdir -p ~/.tauri
tauri signer generate -w ~/.tauri/promptibrary.key
```

The CLI prints the public key on the last line. Copy it into `src-tauri/tauri.conf.json`:

```json
{
  "plugins": {
    "updater": {
      "active": true,
      "endpoints": [
        "https://github.com/scalinity/Promptibrary/releases/latest/download/latest.json"
      ],
      "pubkey": "<paste the public key here>"
    }
  }
}
```

The private key file (`~/.tauri/promptibrary.key`) is **never committed**. Base64-encode it and add it as the GitHub Actions repository secret:

```bash
# macOS — copies the base64 directly to the clipboard.
base64 -i ~/.tauri/promptibrary.key | pbcopy

# Linux fallback — pipe to xclip or just print and copy manually.
base64 ~/.tauri/promptibrary.key
```

Then go to **GitHub → repo Settings → Secrets and variables → Actions → New repository secret**:

| Name                                     | Value                                                                                       |
|------------------------------------------|---------------------------------------------------------------------------------------------|
| `TAURI_SIGNING_PRIVATE_KEY`              | The base64 blob from the previous step.                                                     |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`     | Only if you set a passphrase when generating the key. Otherwise omit this secret entirely.  |

### Key rotation

If the private key leaks or rotates for any reason:

1. Generate a new keypair (`tauri signer generate`).
2. **Cut a transition release signed by both old and new keys.** Distribute via the existing channel; this is the only release the user can install with the old `pubkey` baked in.
3. Update `tauri.conf.json` `pubkey` to the new public key.
4. Update the GitHub Actions secret `TAURI_SIGNING_PRIVATE_KEY` to the new private key.
5. Cut a normal release signed only by the new key.

Without the transition release, every existing install is stranded — they can't verify any update signed only by the new key. The transition release is the only way to migrate.

## What is **not** in V1

Apple Developer ID code signing and `xcrun notarytool` submission. These are explicit V2 candidates (see `docs/V2-CANDIDATES.md`). V1 ships intentionally unsigned by Apple; the personal-use posture accepts the first-launch Gatekeeper friction documented in `docs/INSTALLING.md`.

When Promptibrary moves to a distributed-app posture, the V2 release workflow will add:

- `APPLE_CERTIFICATE` + `APPLE_CERTIFICATE_PASSWORD` (Developer ID Application certificate, PKCS#12 base64)
- `APPLE_ID` + `APPLE_PASSWORD` (notarization credentials; the password is an app-specific password, not the Apple ID password)
- `APPLE_TEAM_ID`
- `xcrun notarytool submit --wait` with a 30-minute timeout, then `xcrun stapler staple` of the resulting ticket onto the `.dmg`

None of those secrets exist in CI today. Adding them is a deliberate scope expansion, not a release-blocker.
