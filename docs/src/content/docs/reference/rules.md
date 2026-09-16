---
title: rules
description: How the work is done: the rituals of the line, and of one project.
---

```
rigger rules [<project>] [--json]
```

Prints the rituals: how work is done here. The line's first, then the ones only that project states about itself.

Both are documents in the record, written with [`rigger doc`](/rigger/reference/doc/). The split is the point. What every project does the same way — a stage ends in a tag, English is what ships, the gate is green before a commit — is written **once**, against the profile. What only one project can say — the crates it has to read a changelog for, the stand it deploys to, the live run it is not finished without — is written against that project.

## Why one place

Before this, the rituals were pasted into every project's skill file. Seventeen projects, seventeen copies, and a change to the line's rituals was seventeen edits — or, more often, one edit and sixteen skills quietly out of date. A skill is now a few lines that say *ask the record*, and `rules` is the asking.

## Reading

```console
$ rigger rules rigger
# The defaults of the line

A stage is a version is a tag. There is no "work on it a bit".
...

# Rituals of rigger

The truth is in the database; hubs are an export.
...
```

Without a project, only the line's:

```console
$ rigger rules
```

A rule of the line is not a rule of whichever project was asked about, so the two are never merged into one text: they are printed one after the other, each as it was written.

## Writing

The line's rituals hang off a project named after the profile, which the record makes the first time something is filed against it:

```console
$ rigger doc add line "Rituals of the line" --kind rituals
$ rigger doc edit line rituals
```

A project's own:

```console
$ rigger doc add rigger "Rituals of rigger" --kind rituals
```

`rituals` is a kind a project has at most one of, so a second is refused by name rather than quietly made. `rules` looks the document up **by kind**, not by address — an address comes from the title, and a Russian title or an imported file gives an address nobody would guess.

## As JSON

```console
$ rigger rules rigger --json
{
  "line": "# The defaults of the line\n...",
  "project": "rigger",
  "project_rules": "# Rituals of rigger\n..."
}
```

`project_rules` is null when that project has written none of its own, and `line` is null when the line has not.

## See also

- [`rigger doc`](/rigger/reference/doc/) — writing and editing the documents `rules` prints
- [`rigger context`](/rigger/reference/context/) — the state of the work, which the rituals do not repeat
- [`rigger skill`](/rigger/reference/skill/) — the skill file, which now points at `rules` instead of carrying a copy
