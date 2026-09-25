---
title: profile
description: One binary, several ways of working - each with a record of its own.
---

```
rigger profile list [--json]
rigger profile show [<NAME>] [--json]
rigger profile use <NAME>
rigger profile add <NAME> [--kind line|tickets] [--root <DIR>]... [--hubs <DIR>] [--id-pattern <REGEX>] [--trays <DIR>] [--use]
rigger profile set [<NAME>] [--root <DIR>]... [--hubs <DIR>] [--id-pattern <REGEX>] [--trays <DIR>]
```

At home a project is a repository with a plan, and the unit of work is a version that ends in a tag. At work the unit is a ticket: one id, a branch in each of several repositories, an inbox the tickets arrive in. None of it belongs in the same record as the line. A profile is what tells the two apart - its own database, its own roots, its own way of naming work - and every command reads the current one without being told.

```console
$ rigger profile list
* line  line     C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\line\rigger.db
  work  tickets  C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\work\rigger.db
```

## The config

Profiles live in `config.toml` in the data directory - a file a person edits:

```toml
current = "line"

[profiles.line]
kind = "line"
roots = ["C:\\dev"]
hubs = "C:\\notes\\Projects"

[profiles.work]
kind = "tickets"
roots = ["C:\\work"]
id_pattern = "[A-Z]{2,8}-\\d+"
trays = "C:\\work\\trays"
```

| Field | What it is for |
| --- | --- |
| `kind` | what the unit of work is: `line` (a version, a tag) or `tickets` (an id, a branch) |
| `roots` | directories whose children are repositories; what [`adopt`](/rigger/reference/adopt/) walks when told nothing |
| `hubs` | the directory whose children are hubs, one per project name; `adopt`'s `--hubs` when told nothing |
| `id_pattern` | how a ticket id is spelt, as a regular expression |
| `trays` | where the cards' [trays](/rigger/reference/tray/) are kept - the material that arrives for a task; `trays/` in the profile's directory when it is not said. Read under its first name, `inbox`, too, and written back as `trays` |

`id_pattern` is recorded now and read by the task commands of the releases that follow; `trays` is read by [`tray`](/rigger/reference/tray/) since v0.25.0; `kind` says which way of working the profile is, and the rest of the surface grows around it. The profile is configuration, not a branch of the code.

## Which profile

`current` in the config, unless `RIGGER_PROFILE` names another - for a shell, a script or a hook that must not depend on what was last switched to:

```console
$ RIGGER_PROFILE=work rigger doctor
profile:   work (C:\Users\you\AppData\Local\lacodda\rigger\data\config.toml)
...
```

`profile use` writes `current`; `profile add --use` adds and switches in one step. `profile set` changes what a profile says about itself - the fields given, and only those; roots given replace the roots it had:

```console
$ rigger profile set --root C:\dev --hubs C:\notes\Projects
Profile 'line' updated
line
  kind:      line
  database:  C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\line\rigger.db
  roots:     C:\dev
  hubs:      C:\notes\Projects
  config:    C:\Users\you\AppData\Local\lacodda\rigger\data\config.toml
```

## Where the database is

Each profile keeps its database under `profiles/<name>/` in the data directory, and its [skill template](/rigger/reference/skill/) there too when it has one of its own. [`backup`](/rigger/reference/backup/) copies the current profile's database beside it.

The one database rigger kept before profiles existed belongs to the default profile, `line`. The first profile-aware rigger to open it moves it into place and keeps the old file beside the config as `rigger.db.before-profiles`, so the move can be undone by hand if it ever has to be:

```
Moved the database into the 'line' profile: C:\Users\you\AppData\Local\lacodda\rigger\data\profiles\line\rigger.db
The file it was is kept as C:\Users\you\AppData\Local\lacodda\rigger\data\rigger.db.before-profiles
```

`init` on a fresh install writes the config with the `line` profile and creates its database.

## Related

- [`adopt`](/rigger/reference/adopt/) - reads the profile's roots and hubs.
- [`doctor`](/rigger/reference/doctor/) - names the profile in use and where its config is.
- [`task`](/rigger/reference/task/) - the statuses the ticket profile lives on.
