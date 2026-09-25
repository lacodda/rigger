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
| [`rigger import`](/rigger/reference/import/) | v0.2.0 | Read a notes hub into versions, tasks and events; a questionnaire's answers with `--answers` (v0.20.0) |
| [`rigger backup`](/rigger/reference/backup/) | v0.2.0 | Copy the database aside, and keep the last few |
| [`rigger context`](/rigger/reference/context/) | v0.3.0 | Print the context packet for a project |
| [`rigger note`](/rigger/reference/note/), `rigger wish` | v0.3.0 | Record an event or a wish |
| [`rigger resolve`](/rigger/reference/resolve/) | v0.7.0 | Answer a question or sort a wish |
| [`rigger open`](/rigger/reference/open/) | v0.4.0 | Start a Claude Code session with the packet |
| [`rigger mcp`](/rigger/reference/mcp/) | v0.5.0 | Serve the packet and the recording tools over MCP |
| [`rigger sync`](/rigger/reference/sync/) | v0.6.0 | Read tags and commits into facts; the branches and commits of task cards (v0.24.0) |
| [`rigger find`](/rigger/reference/find/) | v0.8.0 | Search every project's events |
| [`rigger why`](/rigger/reference/why/) | v0.8.0 | The work that went into a version |
| [`rigger inbox`](/rigger/reference/inbox/) | v0.9.0 | Questions waiting for your answer |
| [`rigger digest`](/rigger/reference/digest/) | v0.9.0 | What moved lately, five lines per project |
| [`rigger version`](/rigger/reference/version/) | v0.10.0 | Aim a version at a week of the calendar |
| [`rigger calendar`](/rigger/reference/calendar/) | v0.10.0 | Weeks by projects: planned, shipped, slipped |
| [`rigger next`](/rigger/reference/next/) | v0.10.0 | This week's focus, and what is behind |
| [`rigger week`](/rigger/reference/week/) | v0.11.0 | The Monday brief: focus, Friday, what waits |
| [`rigger release-day`](/rigger/reference/release-day/) | v0.11.0 | The shopfront queue: what is out, what waits for Friday |
| [`rigger retro`](/rigger/reference/retro/) | v0.12.0 | Planned against shipped, per cycle |
| [`rigger session`](/rigger/reference/session/) | v0.13.0 | Start and end a sitting; the end writes the journal |
| [`rigger export`](/rigger/reference/export/) | v0.14.0 | Write a hub back out of the record |
| [`rigger adopt`](/rigger/reference/adopt/) | v0.15.0 | Record every repository under a directory, with its hub and its tags |
| [`rigger skill`](/rigger/reference/skill/) | v0.15.0 | Write a thin project skill from a template and the record |
| [`rigger profile`](/rigger/reference/profile/) | v0.17.0 | Switch, list and add profiles, each with a record of its own |
| [`rigger task`](/rigger/reference/task/) | v0.17.0 | Task cards: find, make, open, link, write against, close; a status for any task (v0.18.0); tickets from kasl's inbox, `incoming` and `take` (v0.25.0) |
| [`rigger doc`](/rigger/reference/doc/) | v0.19.0 | The handwritten texts of a project: vision, rituals, research; skeletons by kind |
| [`rigger rules`](/rigger/reference/rules/) | v0.20.0 | How the work is done: the rituals of the line, and of one project |
| [`rigger gate`](/rigger/reference/gate/) | v0.20.0 | Run the command that says a project is fit to commit, and record how it went |
| [`rigger show`](/rigger/reference/show/) | v0.21.0 | The project's screen: what it is, where it stands, what it has written down |
| [`rigger link`](/rigger/reference/link/) | v0.22.0 | Tie two projects together, and see when a pair has parted company |
| [`rigger version show`](/rigger/reference/version/#version-show) | v0.23.0 | One version whole: its number, its week, how far it got; the JSON is a contract |
| [`rigger session draft`](/rigger/reference/session/#session-draft) | v0.23.0 | The diary entry the open sitting would leave, to edit before `end --entry` |
| [`rigger tray`](/rigger/reference/tray/) | v0.25.0 | A card's tray: the material that arrives for a task - fetched, taken in, sorted into the card's archive |

## Common to all commands

- `--version` and `--help` on the bare binary; `--help` on every command.
- `RIGGER_DATA_DIR` overrides the data directory for every command; `RIGGER_PROFILE` names the [profile](/rigger/reference/profile/) to use over the one the config points at.
- A record under `RIGGER_DATA_DIR` is a scratch one, and reaches nothing outside its own files - no toast, no GitHub, no kasl - unless the program is named: `RIGGER_NOTIFY` for the [week's toast](/rigger/reference/week/#the-week-as-a-toast), `RIGGER_GH` for [`sync`](/rigger/reference/sync/#releases-and-publishing), `RIGGER_KASL` for [sessions](/rigger/reference/session/#told-to-kasl) and [tickets](/rigger/reference/task/#tickets-from-kasls-inbox), `RIGGER_INTAKE_DIRS` for [`tray intake`](/rigger/reference/tray/#tray-intake).
- The first command of a new week shows the [week as a toast](/rigger/reference/week/#the-week-as-a-toast).
- `--json` prints the same facts as data, on the commands that show facts.
- Exit status is `0` on success and `1` on any failure, with the reason on stderr prefixed `error:`. Never `2`, including for a usage error: an assistant's `Stop` hook reads 2 as a refusal to stop, and a mistyped hook must be ignored rather than hold a session open.
