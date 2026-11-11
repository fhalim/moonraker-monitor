# AGENTS.md

Tauri 2 + React/TS. Rust backend polls Moonraker and sends notifications; UI only renders.

- Rust: `src-tauri/src/` (`moonraker.rs` client/parsing, `monitor.rs` poll loop + `detect_events`, `tray.rs`, `config.rs`, `lib.rs` commands). UI: `src/`.
- Check: `pnpm exec tsc --noEmit`; `cd src-tauri && cargo test`.
- Run: `pnpm tauri dev`. Build: `pnpm tauri build`.
- Windows: set `RUSTC_WRAPPER=''` if the global cargo `kache` wrapper breaks cc-rs builds.
- Keep `@tauri-apps/api`/`cli` versions on the same minor as the `tauri` crate.
- Never send control commands (pause/cancel/e-stop/gcode) to a real printer while testing.
- Closing the window must hide to tray, not exit.
- Release: bump `version` in `tauri.conf.json`, `src-tauri/Cargo.toml`, `package.json`; then `git tag vX.Y.Z && git push origin main vX.Y.Z`. `.github/workflows/release.yml` builds Windows + Linux and publishes the release.
