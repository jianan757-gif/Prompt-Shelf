param([ValidateSet('Prepare','Verify')][string]$Step='Prepare')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$lab=Join-Path $root 'evidence\release-preparation'
$data=Join-Path $env:LOCALAPPDATA 'io.github.promptshelf.stagef.validation'
$install=Join-Path $lab 'installed-validation'
$cli=Join-Path $root 'poc\src-tauri\target-stage-f\release\prompt-shelf-store.exe'
$before=Join-Path $lab 'uninstall-before.json'
New-Item -ItemType Directory -Force -Path $lab|Out-Null
function Audit([string]$name){
  # Read-only audit first: missing/deleted data must fail instead of creating an empty DB.
  $lines=& python (Join-Path $root 'scripts\audit-test-library.py') $data
  if($LASTEXITCODE -ne 0){throw 'Test library missing or invalid; no repair attempted'}
  $text=$lines -join "`n"
  & $cli --data-dir $data --check|Set-Content -Encoding utf8 (Join-Path $lab ($name+'-check.json'))
  if($LASTEXITCODE -ne 0){throw 'Library integrity/hash check failed'}
  [IO.File]::WriteAllText((Join-Path $lab ($name+'.json')),$text,[Text.UTF8Encoding]::new($false))
  return $text
}
if($Step -eq 'Prepare'){
  if(Test-Path -LiteralPath $before){throw 'A prepared run already exists. Preserve it and use -Step Verify after manual uninstall.'}
  if(!(Test-Path -LiteralPath (Join-Path $data 'stage-f-test-owned'))){throw 'Existing F-owned isolated fixture required; do not use personal data'}
  $setup=Join-Path $root 'poc\src-tauri\target-stage-f\release\bundle\nsis\Prompt Shelf F Validation_0.2.0-beta.1_x64-setup.exe'
  if((Get-Item -LiteralPath $setup).VersionInfo.ProductName -ne 'Prompt Shelf F Validation'){throw 'Validation installer required'}
  $p=Start-Process -FilePath $setup -ArgumentList @('/S',('/D='+$install)) -WindowStyle Hidden -PassThru
  if(!$p.WaitForExit(120000) -or $p.ExitCode -ne 0){throw 'Test installation failed or still running; inspect before retry'}
  $null=Audit 'uninstall-before'
  Write-Output "Prepared test identity only. Open: $install\uninstall.exe"
  Write-Output 'Manually CHECK Delete application data, finish uninstall, acknowledge data retention notice, then run this script with -Step Verify.'
}else{
  if(!(Test-Path -LiteralPath $before)){throw 'Prepare baseline missing'}
  if(Test-Path -LiteralPath (Join-Path $install 'prompt-shelf-poc.exe')){throw 'Application is still installed; complete manual uninstall first'}
  $actual=Audit 'uninstall-after'
  if($actual -cne [IO.File]::ReadAllText($before)){throw 'FAIL library or original bytes changed; see before/after evidence'}
  [ordered]@{status='PASS';data='all SQL rows and original SHA-256 unchanged';schema=2;identity='io.github.promptshelf.stagef.validation';checkbox='manual action requires user confirmation; cannot infer it from filesystem';beforeHash=(Get-FileHash $before).Hash;afterHash=(Get-FileHash (Join-Path $lab 'uninstall-after.json')).Hash}|ConvertTo-Json|Set-Content -Encoding utf8 (Join-Path $lab 'uninstall-retention.json')
  Write-Output 'PASS: database, draft/trash rows and original files retained. Please confirm that you checked Delete application data.'
}
