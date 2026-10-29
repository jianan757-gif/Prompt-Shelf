param(
  [Parameter(Mandatory=$true)][string]$OldInstaller,
  [Parameter(Mandatory=$true)][string]$NewInstaller
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$lab=Join-Path $root 'evidence\stage-f'
$install=Join-Path $lab 'installed-validation'
$data=Join-Path $env:LOCALAPPDATA 'io.github.promptshelf.stagef.validation'
$cli=Join-Path $root 'poc\src-tauri\target-stage-f\release\prompt-shelf-store.exe'
$oldCli=Join-Path $root 'poc\src-tauri\target-stage-e\release\prompt-shelf-store.exe'
# Build both packages using the documented test identity, never the real identity.
foreach($file in @($OldInstaller,$NewInstaller)){
  if(!(Test-Path -LiteralPath $file)){throw 'Missing validation installer'}
  if((Get-Item -LiteralPath $file).VersionInfo.ProductName -notlike '*F Validation*'){throw 'Only dedicated F Validation installers are permitted'}
}
if(Test-Path -LiteralPath $data){throw 'Existing validation data; preserve and inspect instead of overwriting'}
New-Item -ItemType Directory -Path $data | Out-Null
[IO.File]::WriteAllText((Join-Path $data 'stage-f-test-owned'),'F installer isolated library')
& $oldCli --data-dir $data --import-manifest (Join-Path $root 'fixtures\100x500\manifest.json') --check | Set-Content -Encoding utf8 (Join-Path $lab 'installer-seed.json')
if($LASTEXITCODE -ne 0){throw 'Test seed failed'}
& python (Join-Path $root 'scripts\seed-installation-state.py') $data
if($LASTEXITCODE -ne 0){throw 'Draft/trash fixture seed failed'}
function Audit([string]$label){
  & $cli --data-dir $data --check | Set-Content -Encoding utf8 (Join-Path $lab ($label+'-check.json'))
  if($LASTEXITCODE -ne 0){throw 'F CLI integrity/hash check failed'}
  $result=& python (Join-Path $root 'scripts\audit-test-library.py') $data
  if($LASTEXITCODE -ne 0){throw 'Data audit failed'}
  $text=$result -join "`n"
  [IO.File]::WriteAllText((Join-Path $lab ($label+'.json')),$text,[Text.UTF8Encoding]::new($false))
  return $text
}
$baseline=Audit 'installer-before'
$steps=[Collections.Generic.List[object]]::new()
function Install([string]$file,[string]$label){
  $process=Start-Process -FilePath $file -ArgumentList @('/S',('/D='+$install)) -WindowStyle Hidden -PassThru
  if(!$process.WaitForExit(120000)){throw 'Installer timed out; inspect process without retry'}
  if($process.ExitCode -ne 0){throw "Installer failed: $($process.ExitCode)"}
  if(!(Test-Path -LiteralPath (Join-Path $install 'prompt-shelf-poc.exe'))){throw 'Installed executable missing'}
  if((Audit $label) -cne $baseline){throw "Data changed at $label"}
  $steps.Add(@{step=$label;exit=$process.ExitCode;data='unchanged';exeVersion=(Get-Item (Join-Path $install 'prompt-shelf-poc.exe')).VersionInfo.ProductVersion})
}
Install $OldInstaller 'after-old-install'
Install $NewInstaller 'after-upgrade'
$uninstaller=Join-Path $install 'uninstall.exe'
if(!(Test-Path -LiteralPath $uninstaller)){throw 'Uninstaller missing'}
$process=Start-Process -FilePath $uninstaller -ArgumentList @('/S',('_?='+$install)) -WindowStyle Hidden -PassThru
if(!$process.WaitForExit(120000)){throw 'Uninstaller timed out; inspect before retry'}
if($process.ExitCode -ne 0){throw 'Uninstall failed'}
if(Test-Path -LiteralPath (Join-Path $install 'prompt-shelf-poc.exe')){throw 'Uninstall left executable'}
if((Audit 'after-uninstall') -cne $baseline){throw 'Uninstall changed data'}
$steps.Add(@{step='uninstall';exit=$process.ExitCode;data='unchanged'})
Install $NewInstaller 'after-reinstall'
$steps | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 (Join-Path $lab 'installer-results.json')
Write-Output 'PASS dedicated test identity install, upgrade, uninstall, reinstall; logical records and original SHA-256 unchanged.'
