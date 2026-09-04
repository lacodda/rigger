---
title: The model
description: Projects, versions, tasks, sessions and events - and where each fact comes from.
---

rigger records five kinds of things. Everything it shows - the digest, the calendar, the inbox, the context packet, the exported hub - is derived from them.

| Entity | What it is | Where it comes from |
| --- | --- | --- |
| **Project** | A repository with a name, a tier and a map of versions | `rigger project add <path>`, then hub import |
| **Version** | A stage that ends in a tag: its items, planned week, tag date | The plan on import; the tag from git on sync |
| **Task** | A unit of work: an item of a version at home, a ticket with an id, aliases and branches at work; may span several projects | The plan, or `rigger task new` |
| **Session** | One conversation of the assistant about a project or a task: start, end, events | `rigger session`, or the MCP server |
| **Event** | A decision, finding, pitfall, change, next step or question to the owner - dated, attributed, tied to a session | MCP recording tools, or `rigger note` |

A **profile** is configuration, not a branch of code: roots, the id pattern, the unit of work (tag or branch), where incoming material lands. Each profile has its own database.

## Facts outrank records

Two sources can describe the same version: the plan and git. When they disagree, git wins and the disagreement is reported:

- a tag exists but the plan still lists the version as open - the version is shipped, and the plan is out of date;
- the plan says a version is closed but no tag exists - the version is not shipped, whatever the plan says.

`rigger sync` reads tags, branches and commits in-process and never runs `git`; `rigger doctor` lists the mismatches.

## The calendar is a view

Tiers, rhythm and the weekly focus are properties of projects and versions. The calendar is a query over them, so it cannot go stale on its own.
