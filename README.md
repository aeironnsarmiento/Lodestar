# Lodestar

A Windows app for running Minecraft Java servers on your own PC. Each server is a
self-contained instance you can create, launch, watch, reset and share from one window —
no batch files, no manual Java installs, no port forwarding.

## Instance management

- **Isolated instances.** Vanilla, Paper, Fabric, Forge (1.17+) and NeoForge servers each
  get their own folder, settings, port and worlds. Run several at once.
- **Guided setup.** Pick a name, type, version (snapshots optional), seed, game mode,
  difficulty, player limit and hardcore. Lodestar downloads and verifies the server files and
  the right Java in the background, with progress on the server's card. Failed setups can be
  retried in place.
- **Managed Java.** The Java version each Minecraft release needs is downloaded as a
  portable Eclipse Temurin runtime and shared between servers. Nothing is installed
  system-wide.
- **Server properties, one panel each.** General (name, MOTD, port, memory), Gameplay (game
  mode, force game mode, difficulty, hardcore, PvP, flight, command blocks), World generation
  (world type, structures, Nether, spawn protection), Players (limit, online mode, signed
  chat, hidden player list, idle kick), Performance (view and simulation distance, entity
  range, safe chunk writes) and Resource pack. Lodestar writes them to `server.properties`
  on every start and keeps any other keys you add by hand.
- **Whitelist and operators.** Turn the whitelist on, choose whether to kick players who are
  not on it, and add or remove names. Operators work the same way, and your own Minecraft
  name is opped every time the server comes online. While a server is online, list changes
  apply straight away.
- **Fabric speed mods.** Fabric servers can get Lithium and FerriteCore installed
  automatically.
- **Mods and plugins tab.** Drag `.jar` files onto the window, or browse Modrinth and
  CurseForge from inside the app (filtered to the server's loader and Minecraft version).
  Installs bring their required dependencies. Turn files on and off, check for and apply
  updates, and remove them. Files added by hand are recognised on Modrinth by their hash.
- **Modpacks.** New server → A modpack: pick a pack on Modrinth or CurseForge, or import a
  `.mrpack` / CurseForge `.zip`. Lodestar installs the exact Minecraft version and loader the
  pack needs, its server-side mods and its settings, and can later switch the server to
  another version of the pack. CurseForge needs your own free API key (Settings →
  CurseForge); files whose authors block app downloads are listed with links to grab them
  by hand.

## Running servers

- **Dashboard** with a card per server: status, live CPU, RAM and player count, its public
  address, and Launch / Stop / Restart / Force kill.
- **Live console** with search, command history and a command line; the history survives
  restarts.
- **Players panel** to op or kick anyone online.
- **Join addresses** for this PC, your LAN, and the internet, each one click to copy.
- **Supervision.** Servers run inside a Windows job object, so closing Lodestar never leaves
  an orphaned `java.exe`. Crashed servers restart automatically.
- **Scheduled restarts** at set times, either with an in-game countdown or waiting until the
  server is empty.
- **Background mode.** Keep-awake while servers run, close to the tray, start with Windows,
  and start chosen servers with the app.

## Worlds

- **Reset World** in one click: players are disconnected and a fresh world boots with a
  random or chosen seed, optionally switching hardcore on or off. Built for speedrun practice.
- Every run gets its own world folder with its seed recorded. The last 10 are kept, so you can
  switch back to any of them from the Worlds tab.

## Playing with friends

Lodestar manages a [playit.gg](https://playit.gg) agent for you. Link your free playit.gg
account once and every server gets a public address the first time it launches — friends
join from anywhere without router setup. Servers that share a port share a tunnel.

## Install

Run `Lodestar_<version>_x64-setup.exe`. It installs for your user only, so no administrator
rights are needed, and adds a Start menu entry.

The installer is not code-signed, so Windows SmartScreen may say it "protected your PC".
Choose **More info → Run anyway**.

## First run

1. **New server** → pick a name, type, version and world options. Lodestar downloads Java
   and the server files in the background; the card shows progress.
2. **Launch**. The first time, Lodestar asks you to accept the
   [Minecraft EULA](https://aka.ms/MinecraftEULA). It asks once for all servers.
3. **Windows Firewall** asks whether Java may accept connections. Allow it on
   **private networks** so people on your Wi-Fi can join.
4. **playit.gg** (sidebar) → **Set up playit.gg**. Your browser opens; sign in or create a
   free playit.gg account and approve Lodestar. Free accounts have a small tunnel limit.

## Where things live

Everything is under `%APPDATA%\Lodestar`:

| Path | What |
|---|---|
| `settings.json` | App settings (theme, tray, start with Windows, EULA acceptance) |
| `instances\<id>\instance.json` | A server's settings |
| `instances\<id>\addons.json` | Where each mod or plugin came from and which version it is |
| `instances\<id>\modpack\` | The modpack file a server was made from |
| `instances\<id>\server\` | Its server files, `server.properties`, mods or plugins |
| `instances\<id>\server\worlds\run_<date>\` | One folder per world, with `seed.txt` |
| `instances\<id>\server\logs\lodestar-console.log` | The console history |
| `runtimes\java\jre-<N>\` | Shared Java runtimes |
| `cache\jars\` | Downloaded server jars, shared between servers |
| `playit\` | The playit.gg agent, its key (`playit.toml`) and logs |

Uninstalling the app leaves this folder; delete it to remove all servers and worlds.

## Development

Built with Tauri 2 (Rust backend, React + TypeScript frontend).

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
LODESTAR_LIVE_TESTS=1 cargo test --manifest-path src-tauri/Cargo.toml live_
```
