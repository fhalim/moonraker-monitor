default: dev

# install/verify all prereqs (bootstrap: install mise, run `mise install` once to get just)
setup: _toolchain _deps _native

_toolchain:
    mise install

_deps:
    pnpm install

[windows]
_native:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/prereqs.ps1

[unix]
_native:
    @echo "no native prereq checks for this platform"

# run desktop app with hot reload (frontend + rust)
dev: setup
    pnpm tauri dev

# production bundle (installer under src-tauri/target/release/bundle)
build: setup
    pnpm tauri build

# frontend-only dev server in browser
web: _deps
    pnpm dev

# typecheck frontend
check: _deps
    pnpm exec tsc --noEmit

# rust lint
rust-check:
    cd src-tauri && cargo clippy

fmt:
    cd src-tauri && cargo fmt

clean:
    cd src-tauri && cargo clean
