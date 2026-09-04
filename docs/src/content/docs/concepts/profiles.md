---
title: Profiles
description: One binary for a product line at home and tickets at work.
---

The same person often keeps two kinds of record. At home, a line of products: each project has a map of versions, and a stage ends in a tag. At work, tickets: a task has an id and aliases, lives in branches, and often touches several repositories at once.

rigger serves both with one binary and one model. What differs is a **profile**:

| Setting | Line profile | Ticket profile |
| --- | --- | --- |
| Unit of work | a version, closed by a tag | a task, tracked by branches |
| Id pattern | none | `[A-Z]{2,8}-\d+`, with aliases when a ticket moves |
| Roots | repositories and the notes hub | repositories and the task cards folder |
| Incoming material | - | a tray per task: screenshots, exports, files from testers |
| Database | its own | its own |

`rigger profile use <name>` switches; every command reads the active profile. A profile is a configuration file, never a code path, so a fix for one is a fix for both.

## No scripts on the work machine

Corporate Windows machines often forbid PowerShell scripts through the execution policy, and hooks that call `.ps1` files stop working without a clear error. rigger is one executable: hooks and automation call the binary directly, and nothing in the workflow depends on a script being allowed to run.
