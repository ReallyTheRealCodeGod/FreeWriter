<#
Removes FreeWriter as installed by install.ps1: the program, its Start menu
and desktop shortcuts, and its entry in Settings > Apps.

Your pages (Documents\Writing) and the log and settings (%APPDATA%\FreeWriter)
are kept.

    powershell -ExecutionPolicy Bypass -File uninstall.ps1
#>

$ErrorActionPreference = 'Stop'
$folder = Join-Path $env:LOCALAPPDATA 'Programs\FreeWriter'
$exe = Join-Path $folder 'freewriter.exe'

if (Get-Process freewriter -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe }) {
    throw 'FreeWriter is running. Finish the session, close it and run this again.'
}

foreach ($place in 'Programs', 'Desktop') {
    $link = Join-Path ([Environment]::GetFolderPath($place)) 'FreeWriter.lnk'
    if (Test-Path $link) { Remove-Item $link -Force }
}
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\FreeWriter'
if (Test-Path $key) { Remove-Item $key -Recurse -Force }

# This script may be running from the folder it removes.
Set-Location $env:TEMP
if (Test-Path $folder) { Remove-Item $folder -Recurse -Force }

Write-Host 'FreeWriter is uninstalled. Your pages, log and settings are still there.'
