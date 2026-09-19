# rigger installer for Windows:
#   irm https://raw.githubusercontent.com/lacodda/rigger/main/tools/install.ps1 | iex
$ErrorActionPreference = "Stop"

$repo = "lacodda/rigger"

# The tag comes from the /releases/latest redirect rather than the REST API:
# unauthenticated API calls are capped at 60 per hour per IP, and an installer
# that fails because someone else on the same address ran it is no installer.
# $env:RIGGER_VERSION pins a specific release.
$tag = $env:RIGGER_VERSION
if (-not $tag) {
    $request = [Net.HttpWebRequest]::Create("https://github.com/$repo/releases/latest")
    $request.AllowAutoRedirect = $false
    $request.UserAgent = "rigger-installer"
    try {
        $response = $request.GetResponse()
        $tag = ($response.Headers["Location"] -split "/")[-1]
        $response.Close()
    } catch {
        throw "Cannot resolve the latest release of ${repo}: $($_.Exception.Message)"
    }
}
if (-not $tag -or $tag -notmatch '^v\d') {
    throw "Cannot resolve the latest release of $repo - set `$env:RIGGER_VERSION to a tag like v0.1.0"
}

$name = "rigger-$tag-x86_64-pc-windows-msvc"
$url = "https://github.com/$repo/releases/download/$tag/$name.zip"
$dir = if ($env:RIGGER_INSTALL_DIR) { $env:RIGGER_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\rigger" }
$tmp = Join-Path ([IO.Path]::GetTempPath()) "rigger-install-$([guid]::NewGuid())"
New-Item -ItemType Directory -Force $tmp | Out-Null

try {
    Write-Host "Downloading $url"
    Invoke-WebRequest $url -OutFile (Join-Path $tmp "rigger.zip")
    # Expand-Archive rather than tar: with Git Bash installed, GNU tar comes
    # first in PATH and chokes on C:\ paths.
    Expand-Archive (Join-Path $tmp "rigger.zip") -DestinationPath $tmp -Force
    New-Item -ItemType Directory -Force $dir | Out-Null
    $exe = Get-ChildItem $tmp -Recurse -Filter "rigger.exe" | Select-Object -First 1
    if (-not $exe) { throw "rigger.exe not found in the downloaded archive" }
    Copy-Item $exe.FullName $dir -Force
} finally {
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}

# Short alias `rgr` as a hard link, not a copy: a copy doubles the install for
# no new code and goes stale the moment an update replaces the binary. A
# symlink would need elevation on Windows; a hard link does not, as long as
# both names are on one volume - and they are, since the alias lands beside
# the binary. Skipped when another `rgr` already answers in PATH;
# $env:RIGGER_NO_ALIAS=1 opts out.
$alias = Join-Path $dir "rgr.exe"
if (-not $env:RIGGER_NO_ALIAS) {
    $existing = Get-Command rgr -ErrorAction SilentlyContinue
    if (-not $existing -or $existing.Source -eq $alias) {
        # A link cannot be created over an existing name.
        Remove-Item $alias -Force -ErrorAction SilentlyContinue
        try {
            New-Item -ItemType HardLink -Path $alias -Target (Join-Path $dir "rigger.exe") -ErrorAction Stop | Out-Null
            Write-Host "Alias rgr -> rigger"
        } catch {
            # A different volume, or a filesystem without hard links: a copy
            # still works, it just has to be refreshed by the installer.
            Copy-Item (Join-Path $dir "rigger.exe") $alias -Force
            Write-Host "Alias rgr -> rigger (copied - this filesystem has no hard links)"
        }
    } else {
        Write-Host "Note: 'rgr' already resolves to $($existing.Source) - alias skipped."
    }
}

# Add the directory to the user PATH in the registry, keeping the value's
# type. PATH is almost always REG_EXPAND_SZ, with entries like %JAVA_HOME%\bin
# stored unexpanded; the .NET environment API rewrites it as a plain string
# and silently breaks every such entry (found on this very installer, v0.1.0). So: read the raw value, compare case-insensitively
# without a trailing slash, write it back as an expandable string, and tell
# running shells about it. A PATH failure must not fail the install.
try {
    $key = Get-Item "HKCU:\Environment"
    $raw = [string]$key.GetValue("Path", "", "DoNotExpandEnvironmentNames")
    $entries = @($raw -split ";" | Where-Object { $_ })
    $wanted = $dir.TrimEnd("\")
    $present = $entries | Where-Object { $_.TrimEnd("\") -ieq $wanted }
    if (-not $present) {
        $value = if ($entries.Count -gt 0) { ($entries + $wanted) -join ";" } else { $wanted }
        Set-ItemProperty -Path "HKCU:\Environment" -Name Path -Value $value -Type ExpandString
        if (-not ("RiggerInstall.Env" -as [type])) {
            Add-Type -Namespace RiggerInstall -Name Env -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("user32.dll", SetLastError = true, CharSet = System.Runtime.InteropServices.CharSet.Unicode)]
public static extern System.IntPtr SendMessageTimeout(System.IntPtr hWnd, uint Msg, System.UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out System.UIntPtr lpdwResult);
'@
        }
        $result = [System.UIntPtr]::Zero
        # HWND_BROADCAST = 0xffff, WM_SETTINGCHANGE = 0x1A, SMTO_ABORTIFHUNG = 0x2
        [RiggerInstall.Env]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, "Environment", 0x2, 5000, [ref]$result) | Out-Null
        Write-Host "Added $dir to your user PATH - open a new terminal to pick it up."
    }
} catch {
    Write-Host "Note: could not update the user PATH ($($_.Exception.Message)); add $dir to it yourself."
}
Write-Host "Installed rigger $tag to $dir\rigger.exe"

# Register the record with the assistant, so that a session can ask it
# rather than being told about it. Two steps, both of which the owner would
# otherwise do by hand and one of which nobody remembers to do at all:
#
#   - the MCP server, which is how an assistant reads the packet and writes
#     decisions back;
#   - the Stop hook, which closes the sitting when the assistant stops. The
#     end-of-session ritual has always been a list an assistant had to
#     remember at exactly the moment it was running out of context, which is
#     when it is least likely to remember anything.
#
# Skipped entirely when the assistant is not installed, and never fatal: an
# install that works is worth more than a registration that is tidy.
# $env:RIGGER_NO_REGISTER=1 opts out.
if (-not $env:RIGGER_NO_REGISTER) {
    $claude = Get-Command claude -ErrorAction SilentlyContinue
    if ($claude) {
        try {
            # The output is kept back; only a failure is worth a line.
            $out = & claude mcp add rigger -- (Join-Path $dir "rigger.exe") mcp 2>&1
            if ($LASTEXITCODE -eq 0) {
                Write-Host "Registered the rigger MCP server with claude."
            } else {
                Write-Host "Note: could not register the MCP server ($out); run: claude mcp add rigger -- rigger mcp"
            }
        } catch {
            Write-Host "Note: could not register the MCP server ($($_.Exception.Message)); run: claude mcp add rigger -- rigger mcp"
        }
    } else {
        Write-Host "Note: claude was not found in PATH, so the MCP server was not registered."
        Write-Host "  Register it later with: claude mcp add rigger -- rigger mcp"
    }

    # The Stop hook, written into the user's settings. `--remind` is the
    # form for a hook: it says nothing unless a sitting was open and
    # something is missing from it, so an assistant stopping in an
    # unrelated directory prints nothing at all.
    #
    # Never exit code 2. An assistant reads 2 from a Stop hook as "refusing
    # to stop" and carries on, and a hook that will not let a session end is
    # worse than no hook (decision of 2026-09-05).
    $settings = Join-Path $env:USERPROFILE ".claude\settings.json"
    $command = "rigger session end --remind"
    try {
        $config = if (Test-Path $settings) { Get-Content $settings -Raw | ConvertFrom-Json } else { [pscustomobject]@{} }
        if (-not $config.PSObject.Properties["hooks"]) { $config | Add-Member hooks ([pscustomobject]@{}) }
        if (-not $config.hooks.PSObject.Properties["Stop"]) { $config.hooks | Add-Member Stop @() }
        $already = @($config.hooks.Stop) | Where-Object { ($_ | ConvertTo-Json -Depth 9) -like "*rigger session end*" }
        if (-not $already) {
            $entry = [pscustomobject]@{ hooks = @([pscustomobject]@{ type = "command"; command = $command }) }
            $config.hooks.Stop = @($config.hooks.Stop) + $entry
            New-Item -ItemType Directory -Force (Split-Path $settings) | Out-Null
            $config | ConvertTo-Json -Depth 9 | Out-File $settings -Encoding utf8
            Write-Host "Added the Stop hook: a sitting now closes itself."
        }
    } catch {
        Write-Host "Note: could not add the Stop hook ($($_.Exception.Message)); add '$command' to hooks.Stop in $settings yourself."
    }
}

Write-Host "Next: run 'rigger init'"
