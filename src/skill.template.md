---
name: {{name}}
description: {{name}} - {{about}}. Code in {{path}}. Triggers on "work on {{name}}", "continue {{name}}", "what is next for {{name}}", and any change under {{path}}.
---

# {{name}}

The record knows where this project stands; the skill only says how to ask it.

## Start

1. `rigger context {{name}}` - the packet: the current stage, what waits on the owner, recent events, the next step. Over MCP it is the `context` tool.
2. Pick up from the next step. Read the hub only when the packet points at it.

## While working

- Record as it happens, never afterwards: `record_decision`, `record_finding`, `record_pitfall`, `record_change` over MCP, or `rigger note {{name}} --kind <kind> "<text>"`.
- A question only the owner can settle goes to `ask_owner`; something wanted later, to `wish`.
- A task of the stage is closed with `close_task`, not by editing the plan: the hub is written from the record by `rigger export`, and an edit there is overwritten.
- Hub: {{hub}}. The vision and the decision log are a person's writing; the rest is a view of the record.
- Repository: {{path}}{{remote}}

## End

Leave the next session a line with `set_next_step`, then `rigger session end {{name}}` - it says what the sitting held and what the ritual still asks for.
