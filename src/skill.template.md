---
name: {{name}}
description: {{name}} - {{about}}. Code in {{path}}. Triggers on "work on {{name}}", "continue {{name}}", "what is next for {{name}}", and any change under {{path}}.
---

# {{name}}

The record knows where this project stands and how the work is done here; the skill only says how to ask it.

## Start

1. `rigger session start {{name}}`
2. `rigger context {{name}}` - the packet: the current stage, what waits on the owner, the gate, recent events, the next step. Over MCP it is the `context` tool.
3. `rigger rules {{name}}` - how the work is done: the rituals of the line, and the ones only this project states about itself.

## While working

- Record as it happens, never afterwards: `record_decision`, `record_finding`, `record_pitfall`, `record_change` over MCP, or `rigger note {{name}} --kind <kind> "<text>"`.
- A question only the owner can settle goes to `ask_owner`; something wanted later, to `wish`.
- A task of the stage is closed with `close_task`, not by editing the plan: the hub is written from the record by `rigger export`, and an edit there is overwritten.
- `rigger gate {{name}}` before a commit; it exits non-zero when the gate is red.
- Repository: {{path}}{{remote}}
- Hub: {{hub}} - the vision and the decision log are a person's writing; the rest is a view of the record.

## End

Leave the next session a line with `set_next_step`, then `rigger session end {{name}}` - it says what the sitting held and what the ritual still asks for.
