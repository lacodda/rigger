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
Write-Host "Next: run 'rigger init'"
