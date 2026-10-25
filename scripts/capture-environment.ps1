$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$output = Join-Path $root 'evidence'
New-Item -ItemType Directory -Force -Path $output | Out-Null
$envInfo = [ordered]@{
  capturedAt = (Get-Date).ToString('o')
  os = Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber
  cpu = Get-CimInstance Win32_Processor | Select-Object Name, NumberOfLogicalProcessors
  memory = Get-CimInstance Win32_ComputerSystem | Select-Object TotalPhysicalMemory
  gpu = Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion, CurrentHorizontalResolution, CurrentVerticalResolution
  webview = Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\*' | Select-Object name,pv
  rustc = (& rustc --version)
  cargo = (& cargo --version)
  node = (& node --version)
  pnpm = (& pnpm --version)
  toolchain = (& rustup show active-toolchain)
  visualStudio = (& 'C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe' -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
  power = (& powercfg /getactivescheme)
  dpiRegistry = Get-ItemProperty 'HKCU:\Control Panel\Desktop' | Select-Object LogPixels,Win8DpiScaling
  dpiNote = 'Registry only; effective native window DPI must be measured in PoC.'
}
$envInfo | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 (Join-Path $output 'environment.json')
Get-ChildItem -LiteralPath (Join-Path $root 'Prompt Shelf 4.2·v2.0') -File -Recurse |
  Get-FileHash -Algorithm SHA256 | Select-Object Path,Hash |
  ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $output 'reference-hashes.json')
Write-Output 'Saved evidence/environment.json and evidence/reference-hashes.json'
