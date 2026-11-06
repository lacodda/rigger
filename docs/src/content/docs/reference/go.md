---
title: go
description: Work on what comes next - the project, its stage, and your assistant started on it.
---

```
rigger go [PROJECT] [--force]
```

One command for a whole turn. It reads git, asks [`pick`](/rigger/reference/pick/) what comes next, opens a [sitting](/rigger/reference/session/), and starts your coding assistant in that project with one line that says what the sitting is for. When the assistant stops, it says how the turn went and names what comes next.

```console
$ rigger go
Work on widget  v0.2.0 · Plugins
  nothing presses - its last sitting, 2026-10-27, is the longest ago
Starting claude in C:\dev\widget

...the session...

widget v0.2.0 went out.
Next: gadget v1.0.0 - `rigger go`
```

It stops there on purpose: the next project is another `go`, so you see each turn end before the next one begins.

## A turn left unfinished comes back

The line does not move on past a project whose turn is not over. `go` returns to it, says what it holds, and the assistant is told to finish that first:

```console
$ rigger go
Work on widget  v0.2.0 · Plugins
  not finished: 3 changed files not committed, 1 commit not pushed
Starting claude in C:\dev\widget
```

What counts is the [`unfinished`](/rigger/reference/pick/#the-rule) reason of `pick`: work not committed or not pushed, an operation git left halfway, a feature or a fix since the last release, a release that did not get out, a sitting left open. A docs or chore commit after a tag holds nothing up.

## `--force`

Goes past every project left unfinished, to the next one with nothing left over, and says whom it went past. Nothing is changed in the projects it passes.

```console
$ rigger go --force
Going past widget.
Work on gadget  v1.0.0 · Public release
```

## A sitting already open

The record cannot tell a sitting someone is at from one that was abandoned, and two assistants in one working tree is the worse mistake. So `go` starts nothing and says so:

```console
$ rigger go
error: widget has a sitting open since 2026-11-05T14:02:11Z. Finish it there, go back to it with `rigger go widget`, or go past it with `rigger go --force`.
```

A sitting `go` opened itself is closed when the assistant stops. One it joined by name is left for whoever opened it.

## Naming a project

`rigger go <PROJECT>` works on that project whatever the rule says of it, even one it leaves out: that is how work on a [blocked](#when-something-out-of-reach-stops-the-tag) project resumes. It joins a sitting already open. With no stage planned, the assistant is told to plan one.

## When something out of reach stops the tag

An assistant that cannot finish for a reason outside the project - a missing key, a pipeline red for reasons outside it, someone else's broken service - records a blocker instead of leaving the turn unfinished:

```console
$ rigger note widget "The signing key has expired; the release workflow needs a new one." --kind blocker
```

The project leaves the order with that as its reason, so the line goes on without `--force`, and the blocker heads the project's [packet](/rigger/reference/context/). [`rigger resolve`](/rigger/reference/resolve/) clears it, and a later release settles it. Over MCP the tool is [`record_blocker`](/rigger/reference/mcp/#tools).

"Hard", "long" and "not sure how" are not blockers.

## Which assistant

`claude` by default. `RIGGER_ASSISTANT` names another, and any flags you set travel with it:

```bash
RIGGER_ASSISTANT="claude --model opus" rigger go
```

The value is split on spaces, so a path containing spaces belongs on the `PATH` or in a small wrapper script rather than in the variable. On Windows a command installed by npm is a `.cmd` shim; rigger finds it through `PATHEXT` and runs it the way a shell would.

The first message is one line and names the commands to read the rest with, rather than carrying the [packet](/rigger/reference/context/) itself. A batch file cannot be handed a line break in an argument, and Claude Code refuses to start with its input piped, so that is what reaches every assistant on every platform. The session stays yours to talk to.

## JSON

Under `--json` it starts nothing and opens no sitting, and prints what it would start. A program reading JSON is not a person about to work in a terminal.

<!-- json: go -->
| Field | Type | Meaning |
| --- | --- | --- |
| `project` | string or null | The project it would start; `null` when it would start nothing. |
| `version` | string or null | The stage to take to a release; `null` when none is planned. |
| `title` | string or null | That stage's title, or `null`. |
| `reason` | string or null | Why it comes now, as for [`pick`](/rigger/reference/pick/); `null` for a project named outright that the rule leaves out. |
| `unfinished` | object or null | What it holds that has not gone out, in the shape of `pick.unfinished`; `null` when nothing. |
| `unfinished.changed` | integer | Tracked files changed and not committed. |
| `unfinished.untracked` | integer | Files git does not know and does not ignore. |
| `unfinished.unpushed` | integer | Commits on no remote. |
| `unfinished.operation` | string or null | An operation git left halfway: `merge`, `rebase`, `cherry-pick`, `revert`, `bisect` or `am`. |
| `unfinished.untagged` | array | The subjects of the features and fixes since the last release. |
| `unfinished.untagged[]` | string | One change's subject line. |
| `unfinished.undelivered` | string or null | How the last release fell short of getting out, in a few words. |
| `unfinished.sitting` | string or null | When the sitting left open began. |
| `dir` | string or null | The directory the assistant would be started in. |
| `program` | string | The assistant's command: the first word of `RIGGER_ASSISTANT`, or `claude`. |
| `message` | string or null | The first message the assistant would receive. |
| `passed_over` | array | The projects `--force` went past. |
| `passed_over[]` | string | One project's name. |
| `busy` | object or null | The project whose open sitting stops it, when one does. |
| `busy.project` | string | That project. |
| `busy.since` | string | When the sitting began. |
| `nothing` | string or null | Why there is nothing to start, when there is nothing. |

## Exit status

The assistant's exit status becomes rigger's, after the report. A sitting open on the next project, an assistant that cannot be run and a name that is no project are failures, with the reason on stderr.

## Related

- [`pick`](/rigger/reference/pick/) - the same answer, without starting anything.
- [`session`](/rigger/reference/session/) - the sitting it opens.
- [`context`](/rigger/reference/context/) - the packet the assistant reads first.
- [`note`](/rigger/reference/note/) - what a session records as it goes.
