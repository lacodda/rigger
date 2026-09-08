---
title: task
description: Give a task a status.
---

```
rigger task status <PROJECT> <TASK> <STATUS>
```

A task's status is a word from a vocabulary rather than one of two:

| Status | What it says |
| --- | --- |
| `new` | untouched - what a plan's empty box means |
| `active` | being worked on |
| `waiting-handoff` | handed to someone else; nothing to do until they answer |
| `frozen` | set aside on purpose |
| `done` | finished - what a ticked box means |
| `dropped` | struck from the hub; given by [`import`](/rigger/reference/import/), never by hand |

Everything before `done` is open work, and is counted as such by the [context packet](/rigger/reference/context/); the packet and the `plan` tool mark any word but `new` beside the task:

```console
$ rigger task status sample 42 waiting-handoff
Task 42 is now waiting-handoff (was new): ask the API team for the missing field

$ rigger context sample
...
## Current stage: v0.2.0 · Second
- ask the API team for the missing field (waiting-handoff)
- write the adapter
```

The ticket profile lives on these: a ticket handed to another team is neither open nor done, and a desk that could only say one of the two lost exactly the tickets that needed watching. Over MCP the same is `set_task_status`; [`close_task`](/rigger/reference/mcp/) is `done` in one word.

A hub's box does not overrule the word. Reading a hub again leaves an active task active - an empty box means "not done", not "new" - and the export writes the box empty for anything short of `done`.

The rest of the task surface - finding a task by id or title, opening one, moving it between projects - arrives with the releases that follow; this page grows with it.

## Related

- [`mcp`](/rigger/reference/mcp/) - `set_task_status` and `close_task`.
- [`profile`](/rigger/reference/profile/) - the ticket profile these statuses serve.
