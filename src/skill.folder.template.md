---
name: {{name}}
description: {{name}} - {{about}}. The work lives in {{path}}, a folder with no repository. Triggers on "work on {{name}}", "continue {{name}}", "what is next for {{name}}", and any change under {{path}}.
---

# {{name}}

The record knows where this project stands and how the work is done here; the skill only says how to ask it.

## Start

1. `rigger session start {{name}}`
2. `rigger context {{name}}` - the packet: the current stage, what waits on the owner, recent events, the next step. Over MCP it is the `context` tool.
3. `rigger rules {{name}}` - how the work is done: the rituals of the line, and the ones only this project states about itself.

Called with a word after its name (`/{{name}} sync`), the word names a ritual in `rigger rules {{name}}`: run that ritual whole.

## While working

- Record as it happens, never afterwards: `record_decision`, `record_finding`, `record_pitfall`, `record_change` over MCP, or `rigger note {{name}} --kind <kind> "<text>"`.
- A question only the owner can settle goes to `ask_owner`; something wanted later, to `wish`.
- A task of the stage is closed with `close_task`, not by editing the plan: the hub is written from the record by `rigger export`, and an edit there is overwritten.
- The work: {{path}} - the files themselves, with no repository and no commits; git is never asked about this project.
- Hub: {{hub}} - the vision and the decision log are a person's writing; the rest is a view of the record.

## End

Leave the next session a line with `set_next_step`, then `rigger session end {{name}}` - it says what the sitting held and what the ritual still asks for.
