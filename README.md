# TIDAL Discord RPC (Rust rewrite)

A Windows-first Discord Rich Presence client for the TIDAL desktop app.

> **AI disclosure:** This is a quick, small project written primarily by
> OpenAI Codex through user-directed iteration and testing. It is hobby
> software, so please review the code and report issues rather than assuming it
> has received a full professional security or compatibility audit.

Unlike the legacy implementation, this version reads TIDAL's Windows media
session instead of guessing playback from its window title. Discord therefore
gets the real track metadata, play/pause state, duration, and current position.

## Features

- Accurate Discord progress bar based on TIDAL's media timeline
- A persistent paused presence with the exact paused position
- Album artwork from a cached, confidence-checked public catalog lookup
- Automatic Discord reconnects and activity cleanup
- No private TIDAL token or account credentials

## Build and run

Requirements: Windows 10 version 1809 or newer, Rust, the TIDAL desktop app,
and the Discord desktop app.

```powershell
cargo build --release
.\target\release\tidal-discord-rpc.exe
```

Keep the executable running while using TIDAL. The release binary is located at
`target\release\tidal-discord-rpc.exe`. It runs in the notification area: right
click its four-diamond icon to temporarily disable presence or exit cleanly.

### Downloading a release

Open this repository's **Releases** page and download
`tidal-discord-rpc-windows-x64.exe`. No installer or extra DLL is required.

Release downloads include a `.sha256` file for integrity verification:

```powershell
Get-FileHash .\tidal-discord-rpc-windows-x64.exe -Algorithm SHA256
```

By default the app uses the original project's Discord application ID. To use
your own Discord application's name and icon:

```powershell
$env:TIDAL_RPC_DISCORD_APP_ID = "your_application_id"
.\target\release\tidal-discord-rpc.exe
```

## How artwork works

Windows supplies TIDAL's artwork as local bytes, while Discord Rich Presence
requires a public HTTPS image URL. This app searches Apple's public music
catalog for the same title, artist, and album, requires a strong metadata match,
and caches the result for the life of the process. If no confident match exists,
it omits the dynamic image so Discord uses the application's configured icon
instead of showing a broken `?` image.

## Privacy

Track, artist, and album text is sent to Apple's public search endpoint only to
resolve cover artwork. Playback state and position remain local, apart from the
Rich Presence data sent to the locally running Discord client.

## Publishing a release

The included GitHub Actions workflow builds releases automatically. Update the
version in `Cargo.toml`, commit it, and push a matching version tag:

```powershell
git tag v2.0.0
git push origin v2.0.0
```

GitHub will test the project, build the Windows executable, calculate its
SHA-256 checksum, and publish both files on the Releases page.

## Credits and license

This is a ground-up Rust rewrite inspired by Stavros Avramidis's original
unofficial TIDAL Discord Rich Presence project. It is not affiliated with or
endorsed by TIDAL or Discord. Released under the MIT License; see `LICENSE`.
