# GRIT portal releases

Windows x64 NSIS builds use the existing Windows review workflow. The version in package.json, package-lock.json, src-tauri/Cargo.toml, Cargo.lock and tauri.conf.json must agree and be stable for portal publication.

1. Commit/push the reviewed source and wait for `GRIT Work Windows review` to pass. Download its `Grit-Work-Windows-review` artifact; retain the run ID and source revision.
2. Verify the Windows installer, including upgrade behavior in the local Windows VM.
3. Run `python3 scripts/prepare-portal-release.py INSTALLER ABSOLUTE_RELEASE_DIRECTORY --notes 'Actual release changes' --portable PORTABLE_ZIP`. This signs the final installer using Tauri and produces manifest.json with its size, SHA-256 and updater signature. The installer must have been built from the exact version and public key in this checkout.
4. Run `python3 /srv/gritprojects/GitHub/login-gritprojects/scripts/software-release.py validate ABSOLUTE_RELEASE_DIRECTORY`, then `publish` when publication is authorized.
5. Verify the public latest endpoint and read back the private release, checking hashes. Verify authenticated HTTP download with an authorized session when available. Retain failed and ambiguous attempts for inspection; never retry automatically.

The private signing key is kept outside the repository at `~/.local/share/grit-release-keys/grit-work.key` with directory mode 700 and key mode 600. Preserve it securely: replacing the public key breaks updates for installed clients. CI does not receive this key; final artifacts are signed locally. This signature authenticates updater payloads; it is not Windows Authenticode signing.

The portal's optional `files[].updater_signature` field contains Tauri's base64 signature. `GET /api/software.php?action=updater&id=grit-work&version=VERSION&arch=x64` requires a genuine portal session and returns only the current, non-withdrawn signed installer, with Tauri metadata plus SHA-256 and size. Downloads retain the existing authentication and checksum checks. Metadata, installers and signatures remain in private storage.

In the app, **Om → Sök uppdateringar** checks public version metadata. The user confirms the target version, pauses collection and signs in through a separate portal window. The remote window has no native Tauri capabilities. Session cookies stay in Rust/the webview and are sent only to the fixed portal HTTPS API with redirects disabled. The updater verifies the signature and the exact authenticated file checksum, rejects downgrades and checks that the selected version is still current immediately before installation. Installation closes the app on Windows. Portable and other platform builds use the portal's manual download flow.

The first version with this capability must be installed manually; rc.4 has its updater disabled. A genuine interactive portal login and end-to-end upgrade to a later signed version must be verified separately from isolated API/auth tests and installer smoke tests.
