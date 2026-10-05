# Glasscraft

A personal Windows 11 app for hosting Minecraft Java servers, with a Liquid Glass look
and a one-click **Reset World** for speedruns.

- Create Vanilla, Paper, Fabric, Forge (1.17+) and NeoForge servers as isolated instances.
- The right Java downloads automatically (portable Eclipse Temurin; nothing is installed system-wide).
- Dashboard cards with live CPU, RAM and players; a live console with search and commands.
- **Reset World**: players are disconnected, a fresh world (random or chosen seed) boots, and
  the last 10 worlds are kept with their seeds so you can go back to any of them.
- Friends join from anywhere through a fully managed playit.gg tunnel.
- Crash restarts, scheduled restarts, keep-awake while servers run, tray, start with Windows.

## Install

Run `Glasscraft_<version>_x64-setup.exe`. It installs for your user only, so no
administrator rights are needed, and adds a Start menu entry.

The installer is not code-signed, so Windows SmartScreen may say it "protected your PC".
Choose **More info → Run anyway**.

## First run

1. **New server** → pick a name, type, version and world options. Glasscraft downloads Java
   and the server files in the background; the card shows progress.
2. **Launch**. The first time, Glasscraft asks you to accept the
   [Minecraft EULA](https://aka.ms/MinecraftEULA). It asks once for all servers.
3. **Windows Firewall** asks whether Java may accept connections. Allow it on
   **private networks** so people on your Wi-Fi can join.
4. **playit.gg** (sidebar) → **Set up playit.gg**. Your browser opens; sign in or create a
   free playit.gg account and approve Glasscraft. Each server then gets a public address the
   first time it launches. Free accounts have a small tunnel limit (servers that share a
   port share a tunnel).

## Where things live

Everything is under `%APPDATA%\Glasscraft`:

| Path | What |
|---|---|
| `settings.json` | App settings (theme, tray, start with Windows, EULA acceptance) |
| `instances\<id>\instance.json` | A server's settings |
| `instances\<id>\server\` | Its server files, `server.properties`, mods or plugins |
| `instances\<id>\server\worlds\run_<date>\` | One folder per world, with `seed.txt` |
| `instances\<id>\server\logs\glasscraft-console.log` | The console history |
| `runtimes\java\jre-<N>\` | Shared Java runtimes |
| `cache\jars\` | Downloaded server jars, shared between servers |
| `playit\` | The playit.gg agent, its key (`playit.toml`) and logs |

Uninstalling the app leaves this folder; delete it to remove all servers and worlds.

## Development

Requirements: Node 22, Rust (stable MSVC) and the Visual Studio C++ build tools.

```bash
npm install
npm run tauri dev      # run the app
npm test               # frontend tests (Vitest)
npm run typecheck      # tsc --noEmit
cargo test --manifest-path src-tauri/Cargo.toml                       # backend tests
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri build    # NSIS installer in src-tauri/target/release/bundle/nsis
```

Tests never download Minecraft, accept the EULA or call playit.gg: servers are played by the
`fake_mc` helper binary and network calls hit a local fixture server. Checks against the real
services are opt-in:

```bash
GLASSCRAFT_LIVE_TESTS=1 cargo test --manifest-path src-tauri/Cargo.toml live_
```

The design and decisions are in `docs/plans/`.
