---
title: open
description: Start an assistant session in the project, with the packet in hand.
---

```
rigger open <PROJECT> [--print] [--budget <TOKENS>]
```

Starts your coding assistant in the project's directory and hands it the [context packet](/rigger/reference/context/) as its first message. The session begins where the last one stopped, instead of with someone reading notes aloud.

```console
$ rigger open sample
Starting claude in C:\dev\sample with the packet for sample
```

The packet arrives with a line of instruction in front of it, so the assistant treats it as the state of the work rather than as something to summarise, and knows to record what it finds with [`rigger note`](/rigger/reference/note/).

## Which assistant

`claude` by default. `RIGGER_ASSISTANT` names another, and any flags you set travel with it:

```bash
RIGGER_ASSISTANT="claude --model opus" rigger open sample
```

The value is split on spaces, so a path containing spaces belongs in a small wrapper script rather than in the variable.

On Windows a command installed by npm is a `.cmd` shim. rigger finds it through `PATHEXT` and runs it the way a shell would, so the bare name `claude` works even though the process API alone would not find it. A batch file cannot be handed a multi-line argument at all, so there the packet arrives on standard input instead - which is what `claude` and its kin read when no prompt argument is given.

## `--print`

Prints the message instead of starting anything - useful to see what the assistant will receive, or to pipe it somewhere else:

```console
$ rigger open sample --print
This is where the project stands, from rigger. Pick up from the next step; record what you decide or find with `rigger note`, and anything for the owner with `rigger wish`.

# sample
...
```

## Exit status

The assistant's exit status becomes rigger's, so `open` composes in a script the same way the assistant would.

## Related

- [`context`](/rigger/reference/context/) - the packet, on its own.
- [`note`](/rigger/reference/note/) - what a session records as it goes.
