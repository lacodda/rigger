---
title: gate
description: Run the command that says a project is fit to commit, and record how it went.
---

```
rigger gate [<project>] [--check] [--json]
```

Runs the project's gate — the command CI runs — in the project's directory, and records the verdict.

The gate is stated once. A repository with a `release.toml` states it there, under `[gate]`, and its commands run as one line joined with ` && `:

```toml
[gate]
commands = ["cargo fmt --all --check"]
```

A project with no such file, or with one that names no commands, sets it on the project:

```console
$ rigger project set rigger --gate "cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test"
rigger: gate is `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test`
Run it with: rigger gate rigger
```

Without a project name, the one the working directory sits in — so a hook or a shell alias needs to be told nothing.

## Where the gate comes from

`release.toml` is the release engine's file: public, in the repository, read by CI with no record around. So it is the truth for what it states, and every reader takes the gate from it: this command, the [packet](/rigger/reference/context/), [`show`](/rigger/reference/show/) and [`project show`](/rigger/reference/project/). What the record held before the file appeared stays there unused; [`project set --gate`](/rigger/reference/project/#--gate) over a gate in the file is refused, since it would change nothing and read as if it had.

A `release.toml` that cannot be read stops `rigger gate`, `--check` included, rather than running the record's line in its place: that would be a gate the repository no longer states. The screens go on without a gate and say why on standard error.

## Why the record runs it

"The gate is green" used to be something an assistant said at the end of a sitting. Now it is something the record witnessed, with the moment it happened and how long it took:

```console
$ rigger gate rigger

rigger: green in 1 minute 11s: cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Every run is kept, including two that say the same thing. A green run last week and a green run today are two different facts, and only one of them says the gate is green now.

## The exit code

`rigger gate` exits **non-zero when the gate is red**, so it composes: a hook, a script or a shell can act on it without reading the text. The gate's own output goes straight to the terminal — a gate is watched while it runs, and a clippy failure held back and replayed at the end is a worse version of what the terminal already does well.

```console
$ rigger gate rigger
...
error: test failed, to rerun pass `--test status`

rigger: red (exit 101) in 1 minute 11s: cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test
$ echo $?
1
```

## At the end of a sitting

If the last gate run **of this sitting** was red, `rigger session end` says so among the things the ritual asks for:

```console
$ rigger session end rigger
...
The ritual asks for:
  the gate was last red (exit 101) in 1 minute 11s: ... - `rigger gate rigger` to run it again
```

Only this sitting's runs count: a gate that went red a week ago and was never run again says nothing about today, and a reminder that fires for ever is a reminder nobody reads. Run it green again and the reminder is gone.

## `--check`

Prints what would run, and runs nothing:

```console
$ rigger gate sample --check
sample would run, in C:\dev\sample:
  cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## As JSON

```console
$ rigger gate rigger --json
{
  "project": "rigger",
  "command": "cargo test",
  "passed": true,
  "exit_code": 0,
  "seconds": 71
}
```

A run prints this.

<!-- json: gate -->
| Field | Type | Meaning |
| --- | --- | --- |
| `project` | string | The name of the project. |
| `command` | string | The gate line that was run. |
| `passed` | boolean | Whether the gate exited zero. |
| `exit_code` | integer | The exit code of the gate. |
| `seconds` | integer | How long it took, in whole seconds. |

With `--check` it prints what would run.

<!-- json: gate -->
| Field | Type | Meaning |
| --- | --- | --- |
| `project` | string | The name of the project. |
| `command` | string | The gate line that would run. |
| `dir` | string | The directory it would run in. |

## Notes

The gate is a **shell line**, not a program with arguments: it is handed to `cmd /C` on Windows and `sh -c` elsewhere, so `&&` means what it means in a shell.

A tool that runs a project's build should not live in that build's output. Running a project's gate from a binary inside its own `target/` fails the moment the gate rebuilds that binary — install it, or run a copy from elsewhere.

## See also

- [`rigger project`](/rigger/reference/project/) — `project set --gate` and `--no-gate`
- [`rigger context`](/rigger/reference/context/) — the packet, which prints the gate on one line
- [`rigger session`](/rigger/reference/session/) — the end-of-sitting reminder
