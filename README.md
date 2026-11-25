# ZimnLauncher

A fast, good-looking game launcher for Linux written in Rust with [egui](https://github.com/emilk/egui). Launch Windows games through Wine, plus AppImages, Java games and Android APKs, from a Steam-style library grid.

## Features

- Steam-style grid of portrait cover cards (2:3) with PNG/JPG artwork cropped to fill the card
- Runs `.exe` through Wine, `.AppImage` directly, `.jar` through Java and `.apk` through Waydroid (or your own emulator command)
- Per-game Wine prefix, or one default prefix for everything
- Smooth hover animations, responsive layout that adapts to any window size
- Right-click a card for Play, rename, change/remove cover, Wine prefix and remove
- Search and optional A-Z sorting
- Custom accent color, background color or background image, optional transparent window
- Dark theme, never blocks the UI (file dialogs and games run on background threads)

## Install

### AppImage

Download `zimnlauncher-x86_64.AppImage` from the [Releases](../../releases) page:

```
chmod +x zimnlauncher-x86_64.AppImage
./zimnlauncher-x86_64.AppImage
```

### From source

Requires Rust 1.76+.

```
git clone <this repo>
cd zimnlauncher
cargo run --release
```

On Linux you also need the usual windowing libraries (`libxkbcommon`, `libwayland`, `libX11`, OpenGL), which any desktop already has.

### Build an AppImage yourself

```
./packaging/build-appimage.sh
```

This produces `zimnlauncher-x86_64.AppImage` in the project root.

### Windows

`zimnlauncher.exe` is built by CI (Actions tab, or attached to a release). It is a normal windowed app without a console.

| Type | How it runs on Windows |
| --- | --- |
| `.exe` | Started directly, no Wine involved (the Wine options have no effect) |
| `.jar` | `java` from `PATH` |
| `.AppImage` | Through **WSL2** with WSLg: needs WSL2 and a Linux distro, the file is run with `--appimage-extract-and-run` (if it is on a Windows drive it is copied into `/tmp` in WSL first) |
| `.apk` | Through **ADB** with an Android emulator, an AVD or a device (Waydroid is Linux only) |

Data is stored in `%APPDATA%\zimnlauncher\`.

## Requirements

| To run | You need |
| --- | --- |
| `.exe` games | `wine` in `PATH` (or a custom binary set in Settings) |
| `.jar` games | `java` in `PATH` (or a custom binary set in Settings) |
| `.AppImage` games | nothing, the executable bit is set automatically |
| `.apk` apps | [Waydroid](https://waydro.id) (Linux, Wayland session), or `adb` with an Android emulator/device, or a custom command |

### Android (APK)

With the default **Waydroid** runner, the first launch starts the Waydroid session if needed, installs the APK with `waydroid app install` and starts it. The detected package name is saved, so later launches start the app directly (the right-click menu has **Reinstall on next launch**). Package detection uses `aapt` when available, otherwise it compares the installed app list before and after the install.

With the **ADB** runner (the default on Windows, works on Linux too) the launcher uses `adb install -r` and `adb shell monkey` on a connected device or emulator. If nothing is connected and an **AVD name** is set, it starts `emulator -avd <name>` and waits for boot. For emulators such as BlueStacks or LDPlayer, enable ADB in their settings and put their address in **adb connect**. Install the [Android platform-tools](https://developer.android.com/tools/releases/platform-tools) for `adb` (and optionally the SDK build-tools for `aapt`).

To use another emulator, choose **Custom** in Settings and enter a shell command, for example `adb install -r {apk} && adb shell monkey -p {pkg} 1`.

## Automatic tool download

If Wine (Linux), Java or adb is missing, ZimnLauncher offers to download it the first time you launch something that needs it, then starts the game when it is ready. Turn on **Download missing tools automatically** in Settings to skip the question, or use the **Download** buttons there.

The **Install all missing tools** button in Settings checks for every tool at once and installs what is missing in a single background job. With **Download missing tools automatically** enabled the same check runs on startup, so a fresh system sets itself up without any clicking; already installed tools are left alone.

| Tool | Source | Notes |
| --- | --- | --- |
| Wine | [Kron4ek/Wine-Builds](https://github.com/Kron4ek/Wine-Builds) | Linux x86_64 only, needs `tar` with xz support and the usual system libraries |
| Java | [Eclipse Temurin](https://adoptium.net) 21 JRE | Linux and Windows |
| adb | Google Android platform-tools | Linux x86_64 and Windows |

Downloads go to `~/.config/zimnlauncher/tools/` (`%APPDATA%\zimnlauncher\tools\` on Windows). A tool found in `PATH` or set in Settings is always used first. Waydroid and WSL2 cannot be installed this way because they need system-level setup.

## Usage

1. Click **+ Add game** (or the **+** card) and pick an `.exe`, `.AppImage`, `.jar` or `.apk`.
2. Right-click the card, choose **Change icon...** and pick a cover image. Portrait art such as 600x900 looks best; other aspect ratios are cropped to the center.
3. Left-click a card to launch it.

### Right-click menu

| Item | Description |
| --- | --- |
| Play | Launch the game |
| Name | Rename the game |
| Change icon / Remove icon | Set or clear the cover image |
| Wine prefix... / Reset prefix | Per-game `WINEPREFIX` (exe only) |
| Remove | Remove the game from the library (files are not deleted) |

## Settings

| Setting | Description |
| --- | --- |
| Wine binary | Command used to run `.exe` files, default `wine` |
| Java binary | Command used to run `.jar` files, default `java` |
| APK runner | `Waydroid` (Linux only), `ADB` (emulator or device) or a `Custom` command (use `{apk}` for the file path and `{pkg}` for the package name) |
| adb / emulator settings | Paths to `adb` and `emulator`, optional `adb connect` address and AVD name for the ADB runner |
| Default prefix | `WINEPREFIX` for games without their own prefix |
| Icon size | Target card width, cards shrink automatically in small windows |
| Accent color | Highlight color for buttons and hover effects |
| Background color / image | Solid color or image behind the library, cropped to fill the window |
| Image dimming | Darkens the background image so cards stay readable |
| Transparent window | Makes the background see-through (restart required, needs a compositor) |
| Window opacity | Background opacity when the window is transparent |
| WINEESYNC / WINEFSYNC | Enable the matching Wine sync primitives |
| Silence wine debug output | Sets `WINEDEBUG=-all` |
| DXVK FPS overlay | Sets `DXVK_HUD=fps` |
| Close launcher after start | Exit when a game starts |
| Sort A-Z | Alphabetical order |

## Data

Everything is stored in `~/.config/zimnlauncher/`:

- `games.json` the library
- `settings.json` the settings
- `icons/` copies of the images you picked (covers and background)

Data from the earlier `winelauncher` directory is migrated automatically on first start.

## Releases

Pushing a tag such as `v0.1.0` runs the GitHub Actions workflow in `.github/workflows/release.yml`, which builds the AppImage and the Windows `.exe` and attaches both to a GitHub release.

```
git tag v0.1.0
git push origin v0.1.0
```
