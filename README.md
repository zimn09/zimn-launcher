# ZimnLauncher

A fast, good-looking game launcher for Linux written in Rust with [egui](https://github.com/emilk/egui). Launch Windows games through Wine, plus AppImages and Java games, from a Steam-style library grid.

## Features

- Steam-style grid of portrait cover cards (2:3) with PNG/JPG artwork cropped to fill the card
- Runs `.exe` through Wine, `.AppImage` directly and `.jar` through Java
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

`zimnlauncher.exe` is also built by CI. On Windows there is no Wine involved: `.exe` files are started directly. This build is experimental and the Wine options have no effect there.

## Requirements

| To run | You need |
| --- | --- |
| `.exe` games | `wine` in `PATH` (or a custom binary set in Settings) |
| `.jar` games | `java` in `PATH` (or a custom binary set in Settings) |
| `.AppImage` games | nothing, the executable bit is set automatically |

## Usage

1. Click **+ Add game** (or the **+** card) and pick an `.exe`, `.AppImage` or `.jar`.
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
