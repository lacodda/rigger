#!/bin/sh
# rigger installer for macOS and Linux:
#   curl -fsSL https://raw.githubusercontent.com/lacodda/rigger/main/tools/install.sh | sh
set -eu

REPO="lacodda/rigger"

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
    Darwin-arm64) TARGET="aarch64-apple-darwin" ;;
    # Git Bash, MSYS2 and Cygwin run this script happily on Windows, where it
    # has no business being: the Windows build is installed by install.ps1.
    # Saying so beats the generic "no prebuilt binary" dead end, since a
    # Windows release does exist - just not for this installer.
    MINGW*|MSYS*|CYGWIN*)
        echo "This is the macOS/Linux installer, but you are on Windows ($(uname -s))." >&2
        echo "Install with PowerShell instead:" >&2
        echo "  irm https://raw.githubusercontent.com/$REPO/main/tools/install.ps1 | iex" >&2
        exit 1
        ;;
    *)
        echo "No prebuilt binary for $(uname -s)/$(uname -m); install with: cargo install rigger" >&2
        exit 1
        ;;
esac

# The tag comes from the /releases/latest redirect rather than the REST API:
# unauthenticated API calls are capped at 60 per hour per IP, and an installer
# that fails because someone else on the same address ran it is no installer.
# RIGGER_VERSION pins a specific release.
TAG="${RIGGER_VERSION:-}"
if [ -z "$TAG" ]; then
    LOCATION=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/$REPO/releases/latest" || true)
    TAG="${LOCATION##*/}"
fi
case "$TAG" in
    v[0-9]*) ;;
    *)
        echo "Cannot resolve the latest release of $REPO - set RIGGER_VERSION to a tag like v0.1.0" >&2
        exit 1
        ;;
esac

NAME="rigger-$TAG-$TARGET"
URL="https://github.com/$REPO/releases/download/$TAG/$NAME.tar.gz"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "Downloading $URL"
curl -fsSL "$URL" | tar xz -C "$TMP"

BIN_DIR="${RIGGER_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$BIN_DIR"
install -m 755 "$TMP/$NAME/rigger" "$BIN_DIR/rigger"
echo "Installed rigger $TAG to $BIN_DIR/rigger"

# Short alias `rgr`, unless something else in PATH already answers to that name
# (ours from a previous run does not count). RIGGER_NO_ALIAS=1 skips it.
# A symlink, never a second binary: one set of bytes answers to both names, so
# `rgr` can never fall behind the version `rigger` was updated to. The archive
# carries one binary and no `rgr` of its own.
#
# The name is `rgr` rather than `rr`, which would have matched the mark: `rr`
# is Mozilla's record-and-replay debugger, packaged in every distribution.
if [ -z "${RIGGER_NO_ALIAS:-}" ]; then
    EXISTING=$(command -v rgr 2>/dev/null || true)
    if [ -z "$EXISTING" ] || [ "$EXISTING" = "$BIN_DIR/rgr" ]; then
        ln -sf rigger "$BIN_DIR/rgr"
        echo "Alias rgr -> rigger"
    else
        echo "Note: 'rgr' already resolves to $EXISTING - alias skipped."
    fi
fi

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "Note: add $BIN_DIR to your PATH." ;;
esac

# Register the record with the assistant, so that a session can ask it
# rather than being told about it: the MCP server, which is how an assistant
# reads the packet and writes decisions back, and the Stop hook, which
# closes the sitting when the assistant stops.
#
# The end-of-session ritual has always been a list an assistant had to
# remember at exactly the moment it was running out of context, which is
# when it is least likely to remember anything. A hook does not forget.
#
# Never fatal, and skipped when the assistant is not installed: an install
# that works is worth more than a registration that is tidy.
# RIGGER_NO_REGISTER=1 opts out.
if [ -z "${RIGGER_NO_REGISTER:-}" ] && command -v claude >/dev/null 2>&1; then
    # MSYS_NO_PATHCONV stops Git Bash rewriting the bare `--` separator and
    # anything that looks like a path into a Windows one, which turns the
    # registration into a server named after a directory. Harmless where
    # the variable means nothing.
    if MSYS_NO_PATHCONV=1 claude mcp add rigger -- "$BIN_DIR/rigger" mcp >/dev/null 2>&1; then
        echo "Registered the rigger MCP server with claude."
    else
        echo "Note: could not register the MCP server; run: claude mcp add rigger -- rigger mcp"
    fi

    # The Stop hook. `--remind` is the form for a hook: it says nothing
    # unless a sitting was open and something is missing from it, so an
    # assistant stopping in an unrelated directory prints nothing.
    #
    # Never exit code 2: an assistant reads 2 from a Stop hook as "refusing
    # to stop" and carries on, so a hook that will not let a session end is
    # worse than no hook.
    SETTINGS="$HOME/.claude/settings.json"
    if command -v python3 >/dev/null 2>&1; then
        if python3 - "$SETTINGS" <<'PYTHON'
import json, os, sys

path = sys.argv[1]
command = "rigger session end --remind"
try:
    with open(path, encoding="utf-8") as f:
        config = json.load(f)
except (OSError, ValueError):
    config = {}
hooks = config.setdefault("hooks", {})
stop = hooks.setdefault("Stop", [])
if any("rigger session end" in json.dumps(entry) for entry in stop):
    sys.exit(1)
stop.append({"hooks": [{"type": "command", "command": command}]})
os.makedirs(os.path.dirname(path), exist_ok=True)
with open(path, "w", encoding="utf-8") as f:
    json.dump(config, f, indent=2)
PYTHON
        then
            echo "Added the Stop hook: a sitting now closes itself."
        fi
    else
        echo "Note: python3 was not found, so the Stop hook was not added."
        echo "  Add 'rigger session end --remind' to hooks.Stop in $SETTINGS yourself."
    fi
fi

echo "Next: run 'rigger init'"
