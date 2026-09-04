---
title: Getting Started
description: Install rigger and record the first project.
---

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
npm i -g @lacodda/rigger
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

```console
$ rigger init
Created C:\Users\you\AppData\Local\lacodda\rigger\data\rigger.db (schema version 2)
Next: rigger project add <path>
```

The database lives in the platform's local data directory; `RIGGER_DATA_DIR` overrides it, and [`rigger doctor`](/rigger/reference/doctor/) prints the path in use.

## Record a project

Point rigger at a repository. The project is named after its directory; `--name` overrides.

```console
$ rigger project add C:\dev\sample
Recorded 'sample' at C:\dev\sample
  remote: https://github.com/acme/sample.git

$ rigger project list
sample  C:\dev\sample
```

## Check the state

```console
$ rigger doctor
database:  C:\Users\you\AppData\Local\lacodda\rigger\data\rigger.db
schema:    version 2
projects:  1
versions:  0
tasks:     0
sessions:  0
events:    0
```

## Connect your assistant

The record is worth keeping only if it is written as the work happens. Connect the [MCP server](/rigger/reference/mcp/), and an assistant reads the packet and records what it decides through tools, instead of leaving it in a transcript:

```console
$ claude mcp add rigger -- rigger mcp
Added stdio MCP server rigger with command: rigger mcp to local config
```

## Next steps

- Read the [model](/rigger/concepts/model/): projects, versions, tasks, sessions and events.
- See how the [context packet](/rigger/concepts/context-packet/) replaces reading a hub.
- Understand [profiles](/rigger/concepts/profiles/): one binary at home and at work.
