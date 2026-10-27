<#
Installs Writer for the current user, no admin rights needed:

- builds the release and copies writer.exe to %LOCALAPPDATA%\Programs\Writer
- adds Writer to the Start menu (and to the desktop with -Desktop)
- lists it under Settings > Apps, where it can be uninstalled

Run it again to update an installed Writer.

    powershell -ExecutionPolicy Bypass -File install.ps1 [-Desktop] [-NoBuild]
#>
param(
    # Also put a shortcut on the desktop.
    [switch]$Desktop,
    # Install target\release\writer.exe as it is, without building first.
    [switch]$NoBuild
)

$ErrorActionPreference = 'Stop'
$source = Join-Path $PSScriptRoot 'target\release\writer.exe'
$folder = Join-Path $env:LOCALAPPDATA 'Programs\Writer'
$exe = Join-Path $folder 'writer.exe'

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
if (Get-Process writer -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }) {
    throw 'Writer is running. Finish the session, close it and run this again.'
}

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
New-Shortcut (Join-Path ([Environment]::GetFolderPath('Programs')) 'Writer.lnk')
if ($Desktop) {
    New-Shortcut (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Writer.lnk')
}

# The entry in Settings > Apps (Add or remove programs), for this user only.
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Writer'
New-Item -Path $key -Force | Out-Null
$uninstall = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$folder\uninstall.ps1`""
$texts = @{
    DisplayName     = 'Writer'
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

Write-Host "Writer is installed in $folder."
Write-Host 'Find it in the Start menu, or search for "Writer".'
