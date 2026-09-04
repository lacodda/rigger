---
title: mcp
description: Serve the record to a coding assistant over the Model Context Protocol.
---

```
rigger mcp
```

Serves the record over [MCP](https://modelcontextprotocol.io) on stdin and stdout: the same packet [`context`](/rigger/reference/context/) prints, plus the tools that write back to it. An assistant with this server connected reads where a project stands and records what it decides as it decides it - instead of the session's findings living in a transcript nobody reads again.

This command is not meant to be typed. A client starts it, talks JSON-RPC to it, and stops it when the session ends.

## Connecting

```console
$ claude mcp add rigger -- rigger mcp
Added stdio MCP server rigger to local config
```

The server needs the database, so run [`rigger init`](/rigger/reference/init/) first; without one it exits saying so rather than serving an empty record.

Anything the server writes on stdout is a protocol message, so a diagnostic never goes there. If a client reports the server as broken, its stderr is where the reason is.

## Tools

| Tool | What it does |
| --- | --- |
| `context` | Where the project stands - the packet, as a session's first read |
| `plan` | The stage being built and its open tasks, each with an id |
| `record_decision` | A decision and the reason behind it |
| `record_finding` | Something learnt about the code or the domain |
| `record_pitfall` | A trap worth remembering |
| `record_change` | Something that changed in the product |
| `set_next_step` | The one line the next session starts from; the newest wins |
| `ask_owner` | A question only the owner can settle; it waits in the packet |
| `wish` | Something to sort into the plan later |
| `close_task` | Marks a task of the current stage done |

Every tool takes a `project` - the name [`rigger project list`](/rigger/reference/project/) shows. The recording tools take a `text`; an empty one is refused, because a record with nothing in it helps nobody.

`plan` prints the ids `close_task` takes, which is why it exists beside `context`:

```
sample · v0.3.0 · Search
- [4] full-text index
- [5] a query language
```

A task id belongs to its project: an id from another project's plan is refused rather than closing someone else's line.

## The packet as a prompt and a resource

The same packet is served two more ways, so a session can start without the assistant remembering to call a tool:

- **Prompt `start session`**, taking a `project`. In Claude Code it appears under `/mcp`.
- **Resource `rigger://<project>/context`**, one per recorded project, so a client can browse the record rather than having to know a name to ask for it.

The prompt wraps the packet in the instruction that tells an assistant what it is holding - the state of the work to pick up from, not a document to summarise.

## Errors

A tool that runs and fails answers with its reason and `isError`, so the model reads what went wrong and can act on it:

```
no project named 'ghost'; the projects are listed by `rigger project list`
```

Only a malformed request is a JSON-RPC error - an unknown method, a call without a name. Either way the server keeps going: one bad request does not end a session, and a line that is not JSON at all is reported on stderr and skipped.

## Related

- [`context`](/rigger/reference/context/) - the same packet, on the command line.
- [`note`](/rigger/reference/note/) - the same recording, for the owner and for scripts.
- [`open`](/rigger/reference/open/) - starting a session with the packet already in it.
