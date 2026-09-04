---
title: Commands
description: The rigger command surface, one page per top-level command.
---

The command surface arrives one release at a time. Each release adds its commands here, one page per top-level command, with subcommands on the page of their command.

| Command | Since | Purpose |
| --- | --- | --- |
| [`rigger init`](/rigger/reference/init/) | v0.1.0 | Create the database |
| [`rigger project`](/rigger/reference/project/) | v0.1.0 | Add, list and show projects |
| [`rigger doctor`](/rigger/reference/doctor/) | v0.1.0 | Show the database path, schema version and record counts |
| [`rigger import`](/rigger/reference/import/) | v0.2.0 | Read a notes hub into versions, tasks and events |
| [`rigger backup`](/rigger/reference/backup/) | v0.2.0 | Copy the database aside |
| [`rigger context`](/rigger/reference/context/) | v0.3.0 | Print the context packet for a project |
| [`rigger note`](/rigger/reference/note/), `rigger wish` | v0.3.0 | Record an event or a wish |
| [`rigger resolve`](/rigger/reference/resolve/) | v0.7.0 | Answer a question or sort a wish |
| [`rigger open`](/rigger/reference/open/) | v0.4.0 | Start a Claude Code session with the packet |
| [`rigger mcp`](/rigger/reference/mcp/) | v0.5.0 | Serve the packet and the recording tools over MCP |
| [`rigger sync`](/rigger/reference/sync/) | v0.6.0 | Read tags and commits into facts |
| [`rigger find`](/rigger/reference/find/) | v0.8.0 | Search every project's events |
| [`rigger why`](/rigger/reference/why/) | v0.8.0 | The work that went into a version |
| `rigger inbox`, `rigger digest` | planned v0.9.0 | Questions waiting for the owner; the five-line digest |
| `rigger calendar`, `rigger next` | planned v0.10.0 | Weeks by projects; this week's focus |
| `rigger week`, `rigger release-day` | planned v0.11.0 | The Monday brief; the Friday queue |
| `rigger retro` | planned v0.12.0 | Planned against shipped, per cycle |
| `rigger session` | planned v0.13.0 | Start and end a session; the end writes the journal |
| `rigger export` | planned v0.14.0 | Generate the hub markdown from the database |
| `rigger skill`, `rigger adopt` | planned v0.15.0 | Generate a thin project skill; record a whole directory of repositories |
| `rigger profile`, `rigger task` | planned v0.17.0+ | Switch profiles; find, open and close tasks |

## Common to all commands

- `--version` and `--help` on the bare binary; `--help` on every command.
- `RIGGER_DATA_DIR` overrides the data directory for every command.
- `--json` prints the same facts as data, on the commands that show facts.
- Exit status is `0` on success and `1` on any failure, with the reason on stderr prefixed `error:`.
