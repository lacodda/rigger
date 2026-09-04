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

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (($userPath -split ";") -notcontains $dir) {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$dir", "User")
    Write-Host "Added $dir to your user PATH - restart the terminal to pick it up."
}
Write-Host "Installed rigger $tag to $dir\rigger.exe"
Write-Host "Next: run 'rigger init'"
