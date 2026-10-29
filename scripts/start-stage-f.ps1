param(
  [ValidateSet('empty','synthetic','mixed')][string]$Fixture='empty',
  [string]$DataDirectory=''
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$release=Join-Path $root 'poc\src-tauri\target-stage-f\release'
$exe=Join-Path $release 'prompt-shelf-poc.exe'
$store=Join-Path $release 'prompt-shelf-store.exe'
foreach($file in @($exe,$store)){
  if(!(Test-Path -LiteralPath $file -PathType Leaf)){throw "Stage F build missing: $file. See README.md."}
}
# Do not stop an existing user session or contend for the PoC hotkeys.
$running=Get-CimInstance Win32_Process -Filter "Name='prompt-shelf-poc.exe'" |
  Where-Object {$_.ExecutablePath -and $_.ExecutablePath.StartsWith($root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)}
if($running){throw 'Exit the running Prompt Shelf from its tray menu before launching Stage F.'}
if([string]::IsNullOrWhiteSpace($DataDirectory)){
  $DataDirectory=Join-Path $root "evidence\stage-f\manual-$Fixture"
}
$data=[IO.Path]::GetFullPath($DataDirectory).TrimEnd('\','/')
$checkOutput=& $store --data-dir $data --check
if($LASTEXITCODE -ne 0){throw 'Library check failed; no GUI was launched.'}
$check=$checkOutput|ConvertFrom-Json
if($Fixture -ne 'empty' -and $check.cases -eq 0 -and $check.assets -eq 0){
  $folder=if($Fixture -eq 'mixed'){'100x500-mixed'}else{'100x500'}
  $manifest=Join-Path $root "fixtures\$folder\manifest.json"
  if(!(Test-Path -LiteralPath $manifest -PathType Leaf)){throw 'Fixture missing. Generate synthetic samples first; mixed samples are local only.'}
  $importOutput=& $store --data-dir $data --import-manifest $manifest --check
  if($LASTEXITCODE -ne 0){throw 'Explicit sample import failed; no GUI was launched.'}
  $check=$importOutput|ConvertFrom-Json
}
# The two executable arguments contain quoted paths; Windows paths cannot contain quotes.
$process=Start-Process -FilePath $exe -ArgumentList @('--data-dir',('"'+$data+'"')) -WorkingDirectory $root -WindowStyle Hidden -PassThru
$evidence=Join-Path $root 'evidence\stage-f'
New-Item -ItemType Directory -Force -Path $evidence|Out-Null
$record=[ordered]@{
  startedAt=(Get-Date).ToString('o');revision='4.4-f';pid=$process.Id;exe=$exe
  exe_sha256=(Get-FileHash -LiteralPath $exe).Hash;dataDirectory=$data;fixture=$Fixture
  dataCheck=$check;status='process launched only; GUI and target receipt require manual acceptance'
}
$record|ConvertTo-Json -Depth 5|Set-Content -Encoding utf8 (Join-Path $evidence ('launch-'+(Get-Date -Format 'yyyyMMdd-HHmmss-fffffff')+'.json'))
$record|ConvertTo-Json -Depth 5
Write-Output 'Stage F uses the displayed persistent test directory. No existing library was cleared. Use tray Exit to close.'
