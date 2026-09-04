---
title: project
description: Add, list and show projects.
---

A project is a repository rigger keeps a record of. This release records the repository itself; versions, tasks and events attach to it in the releases that follow.

## `project add`

```
rigger project add <PATH> [--name <NAME>]
```

Records the repository at `PATH`. The name is the directory name, because that is what a project is called in conversation - a manifest often declares something else, such as the crate `sample-cli` for the product `sample`, or a `*-workspace` root. Pass `--name` when the directory is not it either. The `origin` remote is read from the repository's git config when there is one.

```console
$ rigger project add C:\dev\sample
Recorded 'sample' at C:\dev\sample
  remote: https://github.com/acme/sample.git
```

A path recorded before, or a name already taken, is refused with the existing project named in the message.

## `project list`

```
rigger project list [--json]
```

```console
$ rigger project list
sample  C:\dev\sample
```

With `--json`, an array of the objects `project show --json` prints.

## `project show`

```
rigger project show <NAME> [--json]
```

```console
$ rigger project show sample
sample
  path:    C:\dev\sample
  remote:  https://github.com/acme/sample.git
  since:   2026-09-04T07:41:10Z

$ rigger project show sample --json
{
  "id": 1,
  "name": "sample",
  "path": "C:\\dev\\sample",
  "remote": "https://github.com/acme/sample.git",
  "created_at": "2026-09-04T07:41:10Z"
}
```

Timestamps are UTC in RFC 3339.

## Related

- [`init`](/rigger/reference/init/) - the database `project` writes to.
- [`doctor`](/rigger/reference/doctor/) - how many projects are recorded.
