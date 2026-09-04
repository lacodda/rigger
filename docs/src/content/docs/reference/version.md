---
title: version
description: Aim a version at a week of the release calendar.
---

A version is a stage of a project's plan. `import` and `sync` record them; this command is what gives one a place in the [calendar](/rigger/reference/calendar/).

## `version plan`

```
rigger version plan <PROJECT> <VERSION> --week <WEEK>
rigger version plan <PROJECT> <VERSION> --clear
```

Aims a version at a week, or takes it off the calendar.

```console
$ rigger version plan sample v0.2.0 --week 2026-W37
v0.2.0 is aimed at 2026-W37 - the week of 2026-09-11
```

The week is ISO-8601 - `2026-W37`, and a typed `2026-W7` is understood too. The reply names the Friday, since that is the day the release is for.

Only a version the record already holds can be planned. A misspelt number is refused rather than created: a row nothing else knows about would sit in the calendar for ever, matching no plan, no changelog and no tag.

```console
$ rigger version plan sample v9.9.9 --week 2026-W37
error: no version 'v9.9.9' in the record; see `rigger project show`
```

Planning the same week twice says so and changes nothing. `--clear` removes the aim; the version stays in the plan, it simply has no week.

The week is a **plan**, never a claim about what happened. What happened comes from the tag, and where the two disagree the calendar shows the tag and names the distance. Nothing here overwrites a fact read from git ([ADR 0005](https://github.com/lacodda/rigger/blob/main/docs/adr/0005-facts-from-git.md)).

## Related

- [`calendar`](/rigger/reference/calendar/) - the grid this fills in.
- [`next`](/rigger/reference/next/) - one week of it, in full.
- [`sync`](/rigger/reference/sync/) - what reads the tag a plan is measured against.
