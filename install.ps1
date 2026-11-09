param(
    [string]$Repository = $env:SIMPLE_TORRENT_REPO
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

if (-not $Repository) {
    $Repository = 'masteine/torrent-client'
}
if ([Environment]::Is64BitOperatingSystem -eq $false -or $env:PROCESSOR_ARCHITECTURE -eq 'ARM64') {
    throw 'Готова збірка Windows доступна лише для x86_64.'
}

$archive = 'simple-torrent-windows-x86_64.zip'
$url = "https://github.com/$Repository/releases/latest/download/$archive"
$temporaryDir = Join-Path $env:TEMP ("simple-torrent-" + [guid]::NewGuid().ToString())
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\SimpleTorrent'

New-Item -ItemType Directory -Path $temporaryDir | Out-Null
try {
    Write-Host "Завантажую $archive..."
    $archivePath = Join-Path $temporaryDir $archive
    Invoke-WebRequest -Uri $url -OutFile $archivePath
    Expand-Archive -Path $archivePath -DestinationPath $temporaryDir
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Copy-Item -Path (Join-Path $temporaryDir 'simple-torrent-gui.exe') -Destination $installDir -Force

    $startMenuDir = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
    $shortcutPath = Join-Path $startMenuDir 'Простий торрент-клієнт.lnk'
    $shell = New-Object -ComObject WScript.Shell
    $shortcut = $shell.CreateShortcut($shortcutPath)
    $shortcut.TargetPath = Join-Path $installDir 'simple-torrent-gui.exe'
    $shortcut.WorkingDirectory = $installDir
    $shortcut.Save()
    Write-Host "Встановлено: $installDir"
} finally {
    Remove-Item -Path $temporaryDir -Recurse -Force -ErrorAction SilentlyContinue
}
