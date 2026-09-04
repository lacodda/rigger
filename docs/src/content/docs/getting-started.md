---
title: Getting Started
description: Install rigger and record the first project.
---

:::note[Not released yet]
rigger is at stage 0. The installers below become live with the first release, v0.1.0. Until then, build from source:

```bash
cargo install --git https://github.com/lacodda/rigger
```
:::

## Install

One line on Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/lacodda/rigger/main/tools/install.ps1 | iex
```

One line on macOS / Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/lacodda/rigger/main/tools/install.sh | sh
```

:::caution[On Windows, use the PowerShell line]
`install.sh` carries the macOS and Linux builds only. Running it from Git Bash,
MSYS2 or Cygwin stops with a pointer to `install.ps1` rather than installing
anything.
:::

Via npm:

```bash
npm i -g rigger-cli
```

Via cargo:

```bash
cargo install rigger
```

Or download the archive for your platform from [Releases](https://github.com/lacodda/rigger/releases/latest) (Windows x86_64, Linux x86_64, macOS arm64), unpack and put `rigger` on your `PATH`.

### Installer options

Both scripts read two environment variables:

| Variable | Effect |
| --- | --- |
| `RIGGER_VERSION` | Install this tag (e.g. `v0.1.0`) instead of the newest release |
| `RIGGER_INSTALL_DIR` | Where the binary lands; defaults to `%LOCALAPPDATA%\Programs\rigger` on Windows and `~/.local/bin` elsewhere |

## First run

From v0.1.0:

```bash
rigger init            # creates the database and the default profile
rigger project add .   # records the repository you are in
rigger project list
```

The database lives in the platform data directory (`%LOCALAPPDATA%\lacodda\rigger` on Windows, `~/Library/Application Support/lacodda/rigger` on macOS, `~/.local/share/lacodda/rigger` on Linux); `RIGGER_DATA_DIR` overrides it.

## Next steps

- Read the [model](/rigger/concepts/model/): projects, versions, tasks, sessions and events.
- See how the [context packet](/rigger/concepts/context-packet/) replaces reading a hub.
- Understand [profiles](/rigger/concepts/profiles/): one binary at home and at work.
