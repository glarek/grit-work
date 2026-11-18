# Validation — 2026-11-18

- Frontend: `npm run verify:frontend` — 775 tests passed; Svelte check and Vite production build passed.
- New report/archive/CLI and retention checks: `cargo test -p work-review --bin grit-work grit -- --test-threads=1` — 17 passed before adding the separately run, ignored display integration test.
- Screen lock checks: 2 passed, including all-outputs-asleep handling.
- Real capture service on isolated Xvfb display: `grit_virtual_desktop_capture_saves_readable_image` — passed; saved file decoded successfully. This test is ignored in ordinary runs and requires `GRIT_VIRTUAL_DISPLAY_TEST=1`.
- Core library: 189 tests passed, including rejecting corrupt activity rows instead of silently omitting them from an archive export.
- Final `cargo test --workspace --all-targets`: 783 passed, 0 failed, 1 ignored (the display test was run separately and passed). An unchanged upstream retry-deadline test failed in earlier runs but passed in this final full run; no test budget was relaxed.
- `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- Chromium interaction test with synthetic report: no runtime errors; the submit command occurs only after the final confirmation.
- Native Linux application starts and renders in WebKitGTK on an isolated virtual display. The real Omarchy desktop was locked; it was not unlocked for testing. Grit now queries Omarchy's lock status and waits while outputs are asleep.
- Live read-only Spiris test: authentication and all five registries returned HTTP 200 (47 projects, 5 users, 3 activities, 3 time codes, 8 time articles). No real time entries were posted.
- Codex `gpt-6.2-sol`, Grok `grok-4.7-build-fast`, and agy `gemini-4-hydrogen-medium`: each read an identifier present only in a synthetic image and returned report JSON. CLI smoke tests were run visibly in the existing Herdr workspace.
- Secret scan: supplied application ID and secret absent from repository files. Credentials use the OS keyring; saving requires immediate readback verification.

Unverified: Windows/macOS runtime, live Spiris POST/reconciliation, a complete real 24-hour recording cycle, and failure behavior of every network filesystem. Archive interruption, corruption, missing files, path traversal and repeated cleanup were exercised with local filesystem tests. The artifact is a development preview, not a signed production release.
