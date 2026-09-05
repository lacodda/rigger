---
title: session
description: Open and close a sitting, so that its events belong together and the next one knows where it started.
---

```
rigger session start [<project>] [--json]
rigger session end [<project>] [--heading <TEXT>] [--diary <FILE>] [--remind] [--json]
```

A session is one sitting, and everything recorded while it is open belongs to it.

Before this, an event knew its project and its day and nothing else, so "what did we do last time" could only be answered with a window of days — which is wrong in both directions. Two sittings in an evening became one blur; one spanning midnight became two. A session records the boundary instead of guessing at it.

## `session start`

```console
$ rigger session start sample
Session open on sample. Everything recorded now belongs to it.
```

Nothing else has to change. [`note`](/rigger/reference/note/), `wish`, and the [MCP](/rigger/reference/mcp/) recording tools work exactly as before; the record applies the boundary itself.

The name may be left out, in which case it is the project the working directory sits in - or the nearest one above it.

Starting twice joins the sitting rather than splitting it:

```console
$ rigger session start sample
A session on sample is already open, since 2026-09-05T09:12:44Z.
```

An assistant that lost its place, or a hook that fired again, would otherwise orphan half its events under a session nobody ever ends.

## `session end`

```console
$ rigger session end sample
Session on sample closed, open since 2026-09-05T09:12:44Z.

recorded 1 decision, 1 change
next: ship the retro next

Write it into a diary with: rigger session end sample --diary <file>
```

The chronicle [`sync`](/rigger/reference/sync/) reads out of commit messages is not counted: it is what git already says, and counting it would make any session that happened to run `sync` look productive.

## What the ritual asks for

The end-of-session ritual has always been a list in a skill file that the assistant had to remember at exactly the moment it was running out of context — which is when it is least likely to remember anything. `end` checks the two things that list names, and says so when they are missing:

```console
$ rigger session end sample
Session on sample closed, open since 2026-09-05T11:40:02Z.

recorded 1 change

The ritual asks for:
  changes but no decision, finding or pitfall - nothing says why
```

Work with nothing written down about why is work the record cannot explain later. A sitting that only reasoned is not asked to justify itself — the check looks for changes without reasoning, not for reasoning without changes.

This is not a gate: the session closes either way. rigger reports; what to do about it is yours.

## `--diary`

Appends an entry to a diary file, newest first, under whatever preamble the file already has:

```markdown
# Дневник работы

Одна запись на сессию, новые сверху.

---

## 2026-09-05 · v0.13.0 «Сессии»

**Сделано.**

- разрез витрины по дням

**Грабли.**

- мутация поймала слабый тест

**Следующий шаг.** ship the retro next
```

`--heading` names the entry; without one it is titled by its day alone. A file that does not exist yet is created rather than being a reason to fail at the end of a session.

The entry is the session's **own sentences, arranged** — rigger has no opinion about the day, and inventing one would put words in your diary that nobody said. Empty sections are left out rather than printed as bare headings.

## `--remind`

For a hook. Says nothing unless something is missing, and treats "no session was open" as ordinary rather than as a failure:

```console
$ rigger session end sample --remind
Session on sample closed - 1 event:
  changes but no decision, finding or pitfall - nothing says why
```

A reminder that fires on every stop is a reminder nobody reads.

## As a hook

Claude Code fires a `Stop` hook when a session ends. Pointing it at `session end --remind` is what makes the ritual stop depending on the assistant remembering it:

```json
{
  "hooks": {
    "Stop": [
      { "hooks": [{ "type": "command", "command": "rigger session end --remind" }] }
    ]
  }
}
```

in `~/.claude/settings.json`. `Stop` takes no `matcher` - it fires every time.

No project is named, because the hook has none to give: it is handed a working directory and nothing else. It runs **in** the project, and the record already knows every project by its path, so the directory is the name. Directories above are searched too, since a session ends wherever the last command left the shell rather than at the checkout's root.

The hook is a binary, not a script. Corporate Windows machines forbid PowerShell scripts through the execution policy, and a hook calling a `.ps1` stops working without a clear error.

:::caution[Exit codes matter more than usual here]
A `Stop` hook that exits **2** refuses the stop and holds the assistant's turn open. rigger therefore never exits 2 - a usage error is exit 1 like any other failure, so a mistyped hook, or a rigger too old to know the subcommand, is simply ignored rather than wedging every session.

This was found by installing the hook and running it: the rigger on the PATH was one release behind, and `rigger session end --remind` exited 2.
:::

## What the packet gains

The [context packet](/rigger/reference/context/) opens with a line saying where the last sitting stopped and what has happened since:

```
Last session ended earlier today: 2 events recorded, 1 change committed
```

The question an assistant returning to a project actually has is not "what has been going on" but "what changed while I was away", and without this line every session re-reads the same history unable to tell which part of it is new. When nothing has happened since, it says so — which tells the reader that the events below are all from before.

Commits are counted apart from recorded events, because most of what happens to a project is commits, and the two are missed in different ways.

## Related

- [`context`](/rigger/reference/context/) - the packet a session starts from.
- [`note`](/rigger/reference/note/) - what a session fills with.
- [`digest`](/rigger/reference/digest/) - the same question over days rather than sittings.
- [`open`](/rigger/reference/open/) - starting an assistant with the packet in hand.
