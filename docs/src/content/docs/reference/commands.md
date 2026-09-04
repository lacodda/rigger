---
title: Commands
description: The rigger command surface, one page per top-level command.
---

The command surface arrives one release at a time. Each release adds its commands here, one page per top-level command, with subcommands on the page of their command.

| Command | Arrives with | Purpose |
| --- | --- | --- |
| `rigger init` | v0.1.0 | Create the database and the default profile |
| `rigger project` | v0.1.0 | Add, list and show projects |
| `rigger doctor` | v0.1.0 | Show the database path, schema version and plan/git mismatches |
| `rigger import` | v0.2.0 | Import a notes hub into versions, tasks and events |
| `rigger context` | v0.3.0 | Print the context packet for a project or task |
| `rigger note` | v0.3.0 | Record an event from the command line |
| `rigger mcp` | v0.4.0 | Serve the packet and the recording tools over MCP |
| `rigger sync` | v0.5.0 | Read tags, branches and commits into facts |
| `rigger calendar`, `rigger next` | v0.6.0 | Weeks by projects; this week's focus |
| `rigger inbox`, `rigger digest` | v0.7.0 | Questions waiting for the owner; the five-line digest |
| `rigger export` | v0.8.0 | Generate the hub markdown from the database |
| `rigger session` | v0.9.0 | Start and end a session; the end writes the journal |
| `rigger profile`, `rigger task` | v0.10.0 | Switch profiles; find, open and close tasks |

## Common to all commands

- `--version` and `--help` on the bare binary.
- `RIGGER_DATA_DIR` overrides the data directory for every command.
- Exit status is `0` on success and `1` on any failure, with the reason on stderr.
