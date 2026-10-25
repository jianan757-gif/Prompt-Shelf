param([ValidateSet('synthetic','mixed')][string]$Fixture='synthetic')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$exe=Join-Path $root 'poc\src-tauri\target-round7\release\prompt-shelf-poc.exe'
if(!(Test-Path -LiteralPath $exe)){throw 'Round 7 executable missing. See README.md for the current build command.'}
$dataFolder=if($Fixture -eq 'mixed'){'100x500-mixed'}else{'100x500'}
$manifest=Join-Path $root "fixtures\$dataFolder\manifest.json"
if(!(Test-Path -LiteralPath $manifest)){throw 'Fixture manifest missing'}
$knownExecutables=@($exe,(Join-Path $root 'poc\src-tauri\target-round6\release\prompt-shelf-poc.exe'),(Join-Path $root 'poc\src-tauri\target-round5\release\prompt-shelf-poc.exe'),(Join-Path $root 'poc\src-tauri\target-round4\release\prompt-shelf-poc.exe'),(Join-Path $root 'poc\src-tauri\target-round3\release\prompt-shelf-poc.exe'),(Join-Path $root 'poc\src-tauri\target-round2\release\prompt-shelf-poc.exe'),(Join-Path $root 'poc\src-tauri\target\release\prompt-shelf-poc.exe'))
$running=Get-CimInstance Win32_Process -Filter "Name='prompt-shelf-poc.exe'" | Where-Object {$_.ExecutablePath -in $knownExecutables}
if($running){throw 'A previous or current PoC is still running. Choose Exit PoC from its tray menu, then run this command again.'}
$oldManifest=$env:PROMPT_SHELF_MANIFEST
try {
  $env:PROMPT_SHELF_MANIFEST=$manifest
  $process=Start-Process -FilePath $exe -WorkingDirectory $root -WindowStyle Hidden -PassThru
} finally {
  $env:PROMPT_SHELF_MANIFEST=$oldManifest
}
$record=[ordered]@{startedAt=(Get-Date).ToString('o');revision='round7';pid=$process.Id;exe=$exe;exe_sha256=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash;manifest=$manifest;manifest_sha256=(Get-FileHash -LiteralPath $manifest -Algorithm SHA256).Hash;fixture=$Fixture;status='process launched only; not functional acceptance'}
$name='launch-'+(Get-Date -Format 'yyyyMMdd-HHmmss-fffffff')+'.json'
$record|ConvertTo-Json|Set-Content -Encoding utf8 (Join-Path $root "evidence\$name")
$record|ConvertTo-Json -Compress
Write-Output 'Focus a dedicated test input, then press Ctrl+Alt+1. Use tray Exit PoC to terminate.'




