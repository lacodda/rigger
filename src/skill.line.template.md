---
name: {{line}}
description: {{description}}
---

# {{line}}

One skill for every project of the line. The record knows where each one stands and how the work is done there; this skill only says how to ask it.

Name the project in the request, or work in its directory - every path below is recorded, and `rigger` finds the project from the working directory when no name is given.

## The projects

{{projects}}

## Start

1. `rigger session start <project>`
2. `rigger context <project>` - the packet: the current stage, what waits on the owner, the gate, recent events, the next step. Over MCP it is the `context` tool, and `start session` is the prompt that opens a sitting with it.
3. `rigger rules <project>` - how the work is done: the rituals of the line, and the ones only that project states about itself.

## While working

- Record as it happens, never afterwards: `record_decision`, `record_finding`, `record_pitfall`, `record_change` over MCP, or `rigger note <project> --kind <kind> "<text>"`.
- A question only the owner can settle goes to `ask_owner`; something wanted later, to `wish`.
- A task of the stage is closed with `close_task`, not by editing the plan: a hub is written from the record by `rigger export`, and an edit there is overwritten.
- `rigger gate <project>` before a commit; it exits non-zero when the gate is red.
- `rigger show <project>` - the project's screen: what it is, where it stands, what it has written down.
- `rigger find "<query>"` - where something was decided, across every project at once.

## End

Leave the next session a line with `set_next_step`, then `rigger session end <project>` - it says what the sitting held and what the ritual still asks for.
