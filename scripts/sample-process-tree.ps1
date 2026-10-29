param(
  [Parameter(Mandatory=$true)][int]$RootProcessId,
  [Parameter(Mandatory=$true)][string]$Label,
  [int]$Seconds=60,
  [int]$IntervalMs=1000
)
$ErrorActionPreference='Stop'
if ($Seconds -lt 1 -or $IntervalMs -lt 100) { throw 'Invalid sampling duration' }
$root=Split-Path $PSScriptRoot -Parent
$out=Join-Path $root 'evidence\resources'
New-Item -ItemType Directory -Force -Path $out | Out-Null
$stamp=Get-Date -Format 'yyyyMMdd-HHmmss-fffffff'
$safeLabel=$Label -replace '[^a-zA-Z0-9_-]','_'
$prefix=Join-Path $out "$stamp-$safeLabel"
$cpuCount=[Environment]::ProcessorCount
$known=@{}; $previous=@{}; $rows=[Collections.Generic.List[object]]::new(); $totals=[Collections.Generic.List[object]]::new()
$initial=Get-Process -Id $RootProcessId
$rootIdentity=@{pid=$RootProcessId;path=$initial.Path;start_ticks=$initial.StartTime.ToUniversalTime().Ticks;exe_sha256=(Get-FileHash -LiteralPath $initial.Path -Algorithm SHA256).Hash}
$known[$RootProcessId]=$initial.StartTime.ToUniversalTime().Ticks
$watch=[Diagnostics.Stopwatch]::StartNew()
$lastTime=0.0
while($watch.Elapsed.TotalSeconds -lt $Seconds) {
  $sampleStart=$watch.Elapsed.TotalSeconds
  $all=@(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name,ExecutablePath)
  $processInfo=@{};foreach($item in $all){$processInfo[[int]$item.ProcessId]=$item}
  # Retain known descendants after parent closure; verify PID identity with StartTime.
  $valid=@{}
  foreach($processId in @($known.Keys)) {
    $p=Get-Process -Id $processId -ErrorAction SilentlyContinue
    if($p -and $p.StartTime.ToUniversalTime().Ticks -eq $known[$processId]) { $valid[$processId]=$true }
  }
  do {
    $added=$false
    foreach($item in $all) {
      $processId=[int]$item.ProcessId
      if($valid.ContainsKey([int]$item.ParentProcessId) -and !$valid.ContainsKey($processId)) {
        $p=Get-Process -Id $processId -ErrorAction SilentlyContinue
        if($p) { $known[$processId]=$p.StartTime.ToUniversalTime().Ticks; $valid[$processId]=$true; $added=$true }
      }
    }
  } while($added)
  $now=$watch.Elapsed.TotalSeconds
  $delta=$now-$lastTime
  [long]$working=0; [long]$private=0; [double]$cpuSum=0
  foreach($processId in @($valid.Keys)) {
    $p=Get-Process -Id $processId -ErrorAction SilentlyContinue
    if(!$p) { continue }
    $cpuSeconds=$p.TotalProcessorTime.TotalSeconds
    $cpu=$null
    if($previous.ContainsKey($processId) -and $delta -gt 0) { $cpu=[Math]::Max(0.0,($cpuSeconds-$previous[$processId])/$delta/$cpuCount*100);$cpuSum+=$cpu }
    $previous[$processId]=$cpuSeconds
    $working+=$p.WorkingSet64;$private+=$p.PrivateMemorySize64
    $rows.Add([pscustomobject]@{label=$Label; elapsed_s=$now; pid=$processId; parent_pid=$processInfo[$processId].ParentProcessId;exe_path=$processInfo[$processId].ExecutablePath;name=$p.ProcessName; start_ticks=$known[$processId]; working_bytes=$p.WorkingSet64;private_bytes=$p.PrivateMemorySize64;cpu_percent_machine=$cpu})
  }
  $totals.Add([pscustomobject]@{label=$Label; elapsed_s=$now; process_count=$valid.Count;working_bytes=$working;private_bytes=$private;cpu_percent_machine=if($lastTime -gt 0){$cpuSum}else{$null}})
  $lastTime=$now
  $remaining=$IntervalMs-($watch.Elapsed.TotalSeconds-$sampleStart)*1000
  if($remaining -gt 0){Start-Sleep -Milliseconds ([int]$remaining)}
}
$rows | Export-Csv -NoTypeInformation -Encoding utf8 "$prefix-processes.csv"
$totals | Export-Csv -NoTypeInformation -Encoding utf8 "$prefix-totals.csv"
function Quantile($values,$q) { $a=@($values|Sort-Object); if(!$a.Count){return $null}; return $a[[Math]::Max(0,[Math]::Ceiling($a.Count*$q)-1)] }
$summary=[ordered]@{label=$Label; root_pid=$RootProcessId;seconds=$watch.Elapsed.TotalSeconds;samples=$totals.Count;cpu_normalization='100% = all logical processors';private_mib_p50=(Quantile $totals.private_bytes 0.5)/1MB;private_mib_p95=(Quantile $totals.private_bytes 0.95)/1MB;working_mib_p95=(Quantile $totals.working_bytes 0.95)/1MB;cpu_percent_mean=($totals.cpu_percent_machine|Where-Object { $null -ne $_ }|Measure-Object -Average).Average;gpu='not collected';verdict='baseline only, thresholds not approved';method='CIM ancestry, retained descendant PID+start-time identities; sums may double-count shared working pages; private bytes summed'}
$summary.root_identity=$rootIdentity
$summary.scope_limitation='Descendants only. Confirm dedicated WebView2 user-data-dir ownership before application acceptance; unrelated/pre-existing shared processes are not inferred.'
$summary|ConvertTo-Json -Depth 5|Set-Content -Encoding utf8 "$prefix-summary.json"
$summary|ConvertTo-Json -Compress
