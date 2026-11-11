# Moonraker Monitor

[![Release](https://github.com/fhalim/moonraker-monitor/actions/workflows/release.yml/badge.svg)](https://github.com/fhalim/moonraker-monitor/releases/latest)

A small desktop app that watches a [Moonraker](https://moonraker.readthedocs.io/) 3D printer and tells you when something happens, even when the window is closed.

## Features

- **Live status:** print state, file name, progress, elapsed and remaining time, nozzle and bed temperatures.
- **Controls:** Pause, Resume, Cancel print, Home, Firmware restart and Emergency stop. Buttons are only enabled when they make sense for the current print state, and destructive ones ask for confirmation.
- **Webcams:** cameras found on the printer can be selected during setup and stream live on the dashboard (MJPEG only).
- **Desktop notifications** when a print completes, is paused, or errors. You are also notified if the printer firmware (Klipper) goes into error or shutdown, or if the printer stops responding.
- **System tray:** closing the window hides it to the tray. Monitoring and notifications keep running until you choose Quit from the tray menu.

## Using it

1. Start the app. On first launch it asks for your Moonraker base URL, for example `http://printer.local:7125`.
2. Click **Connect**. The app checks that Moonraker answers at that address. If the printer has webcams, choose which to show, then **Save**.
3. The dashboard appears and updates every 2 seconds.

To change the printer later, open **Settings**. **Test notification** sends a sample notification so you can confirm your system shows them.

Left-click the tray icon to show the window. Right-click it for Show and Quit. Hovering over the icon shows the current print and its progress.

The URL is stored in the app's config folder (`config.json`). Notifications are not sent for the first status the app sees after launch, so starting it while a finished print is still on the printer will not raise an alert.

## Download and install

Get the installer for your system from the [latest release](https://github.com/fhalim/moonraker-monitor/releases/latest). Builds are unsigned.

- **Windows:** run `*-setup.exe` (or the `.msi`). If SmartScreen warns, choose *More info*, then *Run anyway*.
- **Linux:**
  - Debian/Ubuntu: `sudo apt install ./moonraker-monitor_*_amd64.deb`
  - Fedora: `sudo dnf install ./moonraker-monitor-*.rpm`
  - Any distro: `chmod +x *.AppImage && ./*.AppImage`

## Build from source

Requirements: Node.js, pnpm, a Rust toolchain, and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform. On Windows that means the MSVC Build Tools and WebView2.

```sh
pnpm install
pnpm tauri dev      # run with hot reload
pnpm tauri build    # produce installers
```

The repository also has a `justfile` (`just dev`, `just build`, `just check`) and a `mise.toml` pinning the toolchain.

## Limitations

- Only Moonraker instances that do not require an API key or login are supported.
- Status is polled every 2 seconds rather than streamed over a websocket.
- Developed and tested on Windows. Linux installers are built by CI but untested on hardware.

## License

No license has been chosen yet.
