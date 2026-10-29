$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$lab=Join-Path $root 'evidence\release-preparation'
$data=Join-Path $lab 'performance-100x500'
$cli=Join-Path $root 'poc\src-tauri\target-stage-f\release\prompt-shelf-store.exe'
$exe=Join-Path $root 'poc\src-tauri\target-stage-f\release\prompt-shelf-poc.exe'
if(Get-Process prompt-shelf-poc -ErrorAction SilentlyContinue){throw 'Normally exit Prompt Shelf from its tray before measuring; no process will be killed'}
New-Item -ItemType Directory -Force -Path $lab|Out-Null
if(!(Test-Path -LiteralPath $data)){
  & $cli --data-dir $data --import-manifest (Join-Path $root 'fixtures\100x500\manifest.json') --check|Out-Null
  if($LASTEXITCODE -ne 0){throw 'Synthetic fixture import failed'}
}
$checkText=& $cli --data-dir $data --check
if($LASTEXITCODE -ne 0){throw 'Fixture integrity check failed'}
$check=$checkText|ConvertFrom-Json
if($check.cases -ne 100 -or $check.assets -ne 500){throw 'Expected unchanged 100-case / 500-image fixture'}
$startMs=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$p=Start-Process -FilePath $exe -ArgumentList @('--data-dir',('"'+$data+'"')) -WindowStyle Hidden -PassThru
[ordered]@{startedAtMs=$startMs;pid=$p.Id;data=$data;exeSha256=(Get-FileHash $exe).Hash;check=$check;stateTransitions='user-operated; not automatically verified';thresholds='not set'}|ConvertTo-Json -Depth 5|Set-Content -Encoding utf8 (Join-Path $lab 'performance-session.json')
function Sample([string]$label){
  & (Join-Path $root 'scripts\sample-process-tree.ps1') -RootProcessId $p.Id -Label $label -Seconds 60
}
Write-Host '1/5 Keep both windows hidden, do not operate Prompt Shelf during this 60-second sample.'
Sample 'final-hidden'
$null=Read-Host '2/5 Open quick panel with Ctrl+Alt+1, leave visible and idle, then press Enter here'
Sample 'final-quick'
$null=Read-Host '3/5 Open library, hide quick panel, leave library visible and idle, then press Enter here'
Sample 'final-library'
$null=Read-Host '4/5 Close library and hide quick panel, then press Enter here (includes 30-second settling time)'
Start-Sleep -Seconds 30
Sample 'final-library-closed'
Write-Host 'Response measurements: repeat Ctrl+Alt+1 / Esc 10 times; in quick search replace query with 001, then photography, then clear, repeating 10 times. Leave each result visible before the next action. No pasting or editing is needed.'
$null=Read-Host 'When finished, press Enter here. Small sample counts will be reported as-is, without a threshold'
# Observe descendants before the user exits, so orphaned WebView2 processes remain tracked.
$job=Start-Job -ScriptBlock {param($sampler,$processId) & $sampler -RootProcessId $processId -Label 'final-exit' -Seconds 60} -ArgumentList (Join-Path $root 'scripts\sample-process-tree.ps1'),$p.Id
Start-Sleep -Seconds 3
Write-Host '5/5 NOW normally exit Prompt Shelf from the tray. This 60-second sample records remaining processes, including known descendants.'
Wait-Job $job|Out-Null
Receive-Job $job -ErrorAction Stop
if($job.State -ne 'Completed'){throw 'Exit sampling job failed; inspect output'}
Remove-Job $job
& python (Join-Path $root 'scripts\summarize-final-baseline.py')
if($LASTEXITCODE -ne 0){throw 'Response summary failed'}
Write-Output 'Recorded local evidence/release-preparation and evidence/resources. No pass threshold was applied.'
