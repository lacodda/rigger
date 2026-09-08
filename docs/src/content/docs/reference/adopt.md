---
title: adopt
description: Record every repository under a directory, with its hub and its tags.
---

```
rigger adopt <ROOT> [--hubs <DIR>] [--check] [--json]
```

Walks a directory and records the checkouts in it - a child with a `.git` inside - as projects. Where a hub of the same name sits under `--hubs`, it is [imported](/rigger/reference/import/); git is read for what shipped, as [`sync`](/rigger/reference/sync/) would. The three commands a project used to take, once for the line.

```console
$ rigger adopt C:\dev --hubs C:\notes\Projects
alpha  recorded     hub: 12 versions new, 31 tasks new   git: 9 versions shipped, 40 changes read
beta   recorded     hub: 4 versions new, 9 tasks new     git: 2 versions shipped, 11 changes read
gamma  no hub

3 repositories: 2 recorded, 0 known, 1 without a hub, 0 skipped; 2 hubs read.
```

One level only. A line keeps its repositories side by side, and a walk that went deeper would find vendored trees and worktrees and call them projects. A directory without a `.git` is not a project and is not mentioned.

## Which hub is whose

`--hubs` names a directory whose children are hubs, one per project, under the project's name: `C:\notes\Projects\alpha` is the hub of the checkout `C:\dev\alpha`. That is where the hubs of a notes vault live, nowhere near the repositories they describe, and guessing a hub beside the repository would read nothing. A directory with the right name and none of the [files a hub is read from](/rigger/reference/import/) is not a hub.

With `--hubs`, the hub is what makes a checkout one of the line. A directory of repositories holds strays - an experiment, a fork kept for reading, a plugin of another product - and recording every one of them would put a dozen projects nobody works on into every list. A checkout without a hub is named and left alone. Without `--hubs`, every checkout is recorded.

The record remembers where each hub is, so that [`doctor --hubs`](/rigger/reference/doctor/) and [`export`](/rigger/reference/export/) can find it later.

## Running it again

A project already recorded is not recorded twice; its hub and its tags are read all the same. So this is the command to run when the line has grown: the new checkout is recorded, the rest say `known`, and nothing that was quiet is reported as news.

```console
$ rigger adopt C:\dev --hubs C:\notes\Projects
alpha  known        hub: nothing new   git: nothing new
beta   known        hub: nothing new   git: 1 version shipped, 3 changes read
delta  recorded     hub: 2 versions new, 5 tasks new   git: nothing new
gamma  no hub

4 repositories: 1 recorded, 2 known, 1 without a hub, 0 skipped; 3 hubs read.
```

A checkout whose name is already taken by a project at another path is skipped and said, rather than recorded under a name that would lie about where it is:

```
alpha  skipped
       a project named 'alpha' is recorded at D:\old\alpha
```

## `--check`

Says what would be recorded and where a hub was found, and writes nothing:

```console
$ rigger adopt C:\dev --hubs C:\notes\Projects --check
alpha  would record hub: found
beta   would record hub: found
gamma  no hub

3 repositories: 2 would be recorded, 0 known, 1 without a hub, 0 skipped; 0 hubs read.
Nothing was written. Run again without --check to record them.
```

## Related

- [`project add`](/rigger/reference/project/) - one project at a time, with a name of your choosing.
- [`import`](/rigger/reference/import/) - what reading a hub does.
- [`sync`](/rigger/reference/sync/) - what reading git does.
- [`skill`](/rigger/reference/skill/) - the other half of moving a project onto rigger.
