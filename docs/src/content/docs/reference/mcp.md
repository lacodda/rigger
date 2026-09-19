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
Added stdio MCP server rigger with command: rigger mcp to local config
```

The installer runs this for you, along with adding the [`Stop` hook](/rigger/reference/session/) that closes a sitting. It is skipped when the assistant is not on the PATH, and never fails the install - registration that is tidy is worth less than an install that works. `RIGGER_NO_REGISTER=1` opts out.

In Git Bash the command needs `MSYS_NO_PATHCONV=1` in front of it: otherwise the shell rewrites the bare `--` separator into a Windows path and the server ends up registered under the name of a directory. The installer does this already.

[`doctor`](/rigger/reference/doctor/) says whether the server answers:

```console
$ rigger doctor
...
mcp:       answers - protocol 2025-06-18, 18 tools, 1 prompt
```

It asks the server rather than starting a process and speaking the protocol down a pipe. "Does the server answer" is a question about this code; a subprocess would test the shell, the PATH and the binary on disk instead - three things that can be wrong while the server is fine, and one, a stale binary earlier in the PATH, that would make a broken install look healthy.

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
| `record_state` | One line for the top of the hub's state block, when the state shifted |
| `set_next_step` | The one line the next session starts from; the newest wins |
| `ask_owner` | A question only the owner can settle; it waits in the packet |
| `wish` | Something to sort into the plan later |
| `resolve` | Answers a question or sorts a wish, so it leaves the packet |
| `close_task` | Marks a task of the current stage done |
| `doc_show` | Reads a handwritten text - the vision, the rituals, a research note; lists them without a slug |
| `doc_write` | Writes one into the record: a research note made on request, or a correction to the vision |
| `set_task_status` | Gives a task a [status](/rigger/reference/task/): new, active, waiting-handoff, frozen or done |
| `task_find` | Finds the [card](/rigger/reference/task/) a line of text means, and says whether to take it, ask, or make a new one |
| `task_context` | Where a card stands: what it is, where it is worked, everything written against it |
| `record_plan` | A step of the plan of edits, against a card |

`doc_write` replaces the whole body of a document, so read it with `doc_show` first and send back the whole of it. What it is not told, it keeps: a rewrite without `kind` or `title` leaves both as they were, so a correction to the vision cannot silently retitle it or turn it into a research note. An empty body is refused - a document is removed with `rigger doc remove`, not by emptying it.

This is what lets an assistant write a research note or fix a vision into the record rather than into a file somebody has to remember to import. See [`doc`](/rigger/reference/doc/).

The recording tools - `record_decision`, `record_finding`, `record_pitfall`, `record_change`, `record_plan` and `set_next_step` - take a `task` instead of a `project`: the event is then written against that card, under the desk the cards live in.

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
- [`resolve`](/rigger/reference/resolve/) - what the `resolve` tool does on the command line.
- [`open`](/rigger/reference/open/) - starting a session with the packet already in it.
