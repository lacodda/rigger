---
title: open
description: Start a Claude Code session with the context packet already in hand.
---

```
rigger open <PROJECT> [--print] [--budget <TOKENS>]
```

Launches Claude Code in the project's directory and passes the context packet as its first message, so the session begins where the last one stopped instead of with someone reading notes aloud.

rigger does not run an assistant itself - it launches whichever command you already use, `claude` by default. Set `RIGGER_ASSISTANT` to run a different one:

```bash
export RIGGER_ASSISTANT="claude --model opus"
rigger open sample
```

## How a session works

When Claude Code starts, it receives this as its first message:

```
This is where the project stands, from rigger. Pick up from the next step; 
record what you decide or find with `rigger note`, and anything for the owner with `rigger wish`.

# sample
C:\dev\sample
https://github.com/acme/sample.git
Last shipped: v0.2.0 on 2026-09-03
1 versions planned, 2 tasks open
...
```

Your assistant now has:
- The exact state the last session left behind
- What is being built this version
- What waits for the owner to decide
- The history of recent decisions and findings
- The one line to start from

During the session, instead of editing notes manually, use these commands:

```bash
# Record a finding, decision, pitfall, or change
rigger note sample "Database schema evolved from one table per version to ..."
rigger note sample "Ship this before refactoring the parser." --kind decision

# Record what you want to do next
rigger note sample "Polish the importer UX." --kind next

# Record something to sort into the plan later
rigger wish sample "Add a --verbose flag to the sync command"
```

Each command adds to the record instantly. The next session reads them from the packet.

## `--print`

Print the first message instead of starting Claude Code:

```bash
rigger open sample --print
```

Useful for reviewing what a session will see, or for piping to another tool.

## `--budget <TOKENS>`

Set the token budget for the packet (default 3000). A smaller budget keeps the session fast; a larger one brings more history:

```bash
rigger open sample --budget 5000
```

## Environment

| Variable | Default | Purpose |
| --- | --- | --- |
| `RIGGER_ASSISTANT` | `claude` | Command to launch, split on spaces so flags travel with it |
| `RIGGER_DATA_DIR` | Platform-dependent (see [`init`](/rigger/reference/init/)) | Directory for the database |

## Related

- [`context`](/rigger/reference/context/) - what the packet contains
- [`note`](/rigger/reference/note/) - record during a session
