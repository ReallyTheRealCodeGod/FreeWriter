<#
Installs FreeWriter for the current user, no admin rights needed:

- builds the release and copies freewriter.exe to %LOCALAPPDATA%\Programs\FreeWriter
- adds FreeWriter to the Start menu (and to the desktop with -Desktop)
- lists it under Settings > Apps, where it can be uninstalled

Run it again to update an installed FreeWriter. An install from when the app
was called Writer is taken away.

    powershell -ExecutionPolicy Bypass -File install.ps1 [-Desktop] [-NoBuild]
#>
param(
    # Also put a shortcut on the desktop.
    [switch]$Desktop,
    # Install target\release\freewriter.exe as it is, without building first.
    [switch]$NoBuild
)

$ErrorActionPreference = 'Stop'
$source = Join-Path $PSScriptRoot 'target\release\freewriter.exe'
$folder = Join-Path $env:LOCALAPPDATA 'Programs\FreeWriter'
$exe = Join-Path $folder 'freewriter.exe'
$oldFolder = Join-Path $env:LOCALAPPDATA 'Programs\Writer'
$oldExe = Join-Path $oldFolder 'writer.exe'

if (-not $NoBuild) {
    Push-Location $PSScriptRoot
    try {
        cargo build --release
        if ($LASTEXITCODE -ne 0) { throw 'The build failed, so nothing was installed.' }
    } finally {
        Pop-Location
    }
}
if (-not (Test-Path $source)) { throw "There's no $source to install. Run without -NoBuild." }
$running = Get-Process freewriter, writer -ErrorAction SilentlyContinue | Where-Object { $_.Path -in $exe, $oldExe }
if ($running) {
    throw 'FreeWriter is running. Finish the session, close it and run this again.'
}

# The same app under its old name, Writer: its program, shortcuts and entry
# in Settings > Apps go, so it isn't there twice. A desktop shortcut comes
# back under the new name.
$oldDesktop = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Writer.lnk'
$Desktop = $Desktop -or (Test-Path $oldDesktop)
foreach ($link in (Join-Path ([Environment]::GetFolderPath('Programs')) 'Writer.lnk'), $oldDesktop) {
    if (Test-Path $link) { Remove-Item $link -Force }
}
$oldKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Writer'
if (Test-Path $oldKey) { Remove-Item $oldKey -Recurse -Force }
if (Test-Path $oldFolder) { Remove-Item $oldFolder -Recurse -Force }

New-Item -ItemType Directory -Force $folder | Out-Null
Copy-Item $source $exe -Force
Copy-Item (Join-Path $PSScriptRoot 'uninstall.ps1') (Join-Path $folder 'uninstall.ps1') -Force

$shell = New-Object -ComObject WScript.Shell
function New-Shortcut([string]$path) {
    $link = $shell.CreateShortcut($path)
    $link.TargetPath = $exe
    $link.WorkingDirectory = $folder
    $link.Description = 'A distraction-free writing app that works like an old typewriter'
    $link.IconLocation = "$exe,0"
    $link.Save()
}
New-Shortcut (Join-Path ([Environment]::GetFolderPath('Programs')) 'FreeWriter.lnk')
if ($Desktop) {
    New-Shortcut (Join-Path ([Environment]::GetFolderPath('Desktop')) 'FreeWriter.lnk')
}

# The entry in Settings > Apps (Add or remove programs), for this user only.
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\FreeWriter'
New-Item -Path $key -Force | Out-Null
$uninstall = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$folder\uninstall.ps1`""
$texts = @{
    DisplayName     = 'FreeWriter'
    DisplayVersion  = (Get-Item $exe).VersionInfo.ProductVersion
    DisplayIcon     = "$exe,0"
    InstallLocation = $folder
    UninstallString = $uninstall
}
foreach ($name in $texts.Keys) {
    New-ItemProperty -Path $key -Name $name -Value $texts[$name] -PropertyType String -Force | Out-Null
}
$numbers = @{
    NoModify      = 1
    NoRepair      = 1
    EstimatedSize = [int][math]::Ceiling((Get-Item $exe).Length / 1KB)
}
foreach ($name in $numbers.Keys) {
    New-ItemProperty -Path $key -Name $name -Value $numbers[$name] -PropertyType DWord -Force | Out-Null
}

Write-Host "FreeWriter is installed in $folder."
Write-Host 'Find it in the Start menu, or search for "FreeWriter".'
