$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$poc=Join-Path $root 'poc'
$target=Join-Path $poc 'src-tauri\target-stage-f'
$evidence=Join-Path $root 'evidence\stage-f'
$oldTarget=$env:CARGO_TARGET_DIR
Get-Command node,cargo -ErrorAction Stop|Out-Null
New-Item -ItemType Directory -Force -Path $evidence|Out-Null
Push-Location $poc
try {
  $env:CARGO_TARGET_DIR=$target
  # The Tauri CLI enables the production resource protocol; plain cargo release does not.
  $config=@{build=@{beforeBuildCommand='node node_modules/typescript/bin/tsc --noEmit && node node_modules/vite/bin/vite.js build'}}|ConvertTo-Json -Compress
  $configFile=Join-Path $evidence 'build-config.json'
  [IO.File]::WriteAllText($configFile,$config,[Text.UTF8Encoding]::new($false))
  # Windows PowerShell 5 treats native stderr progress as ErrorRecords; check exit codes instead.
  $ErrorActionPreference='Continue'
  & node node_modules/@tauri-apps/cli/tauri.js build --bundles nsis --config $configFile -- --locked 2>&1|Tee-Object -FilePath (Join-Path $evidence 'gui-build.txt')
  $guiExit=$LASTEXITCODE
  $ErrorActionPreference='Stop'
  if($guiExit -ne 0){throw 'Tauri production build failed.'}
  $ErrorActionPreference='Continue'
  & cargo build --manifest-path src-tauri/Cargo.toml --release --bin prompt-shelf-store --locked 2>&1|Tee-Object -FilePath (Join-Path $evidence 'store-build.txt')
  $storeExit=$LASTEXITCODE
  $ErrorActionPreference='Stop'
  if($storeExit -ne 0){throw 'Store CLI build failed.'}
  $gui=Join-Path $target 'release\prompt-shelf-poc.exe'
  $cli=Join-Path $target 'release\prompt-shelf-store.exe'
  $product=Get-Content -LiteralPath (Join-Path $poc 'src-tauri\tauri.conf.json') -Raw|ConvertFrom-Json
  $installer=Join-Path $target ('release\bundle\nsis\'+$product.productName+'_'+$product.version+'_x64-setup.exe')
  [ordered]@{
    builtAt=(Get-Date).ToString('o')
    installerSha256=(Get-FileHash -LiteralPath $installer).Hash
    guiSha256=(Get-FileHash -LiteralPath $gui).Hash
    storeSha256=(Get-FileHash -LiteralPath $cli).Hash
    status='production build only; runtime acceptance is separate'
  }|ConvertTo-Json|Set-Content -Encoding utf8 (Join-Path $evidence 'build.json')
} finally {
  $env:CARGO_TARGET_DIR=$oldTarget
  Pop-Location
}
