param([switch]$BuildOnly,[switch]$Release,[switch]$LaunchOnly,[string]$ServicePackages)
$ErrorActionPreference='Stop'
if($BuildOnly -and $LaunchOnly){throw 'BuildOnly and LaunchOnly cannot be combined.'}
$repoRoot=Split-Path -Parent $PSScriptRoot
$profile=if($Release){'release'}else{'debug'}
$target=if($env:CARGO_TARGET_DIR){[IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)}else{Join-Path $repoRoot 'target'}
$application=Join-Path $target "$profile/lazybuddy.exe"
$cli=Join-Path $target "$profile/lazybuddy-cli.exe"
Push-Location $repoRoot
try{
    if(!$LaunchOnly){
        if(!(Test-Path apps/desktop/node_modules/.bin/vite.cmd)){
            npm.cmd --prefix apps/desktop ci --legacy-peer-deps --no-audit --no-fund
            if($LASTEXITCODE){throw 'Frontend dependency installation failed.'}
        }
        Write-Host '[1/2] Building LazyBuddy Web UI...'
        npm.cmd --prefix apps/desktop run build
        if($LASTEXITCODE){throw 'Frontend build failed; no old binary will be launched.'}
        Write-Host '[2/2] Building LazyBuddy host, CLI and updater...'
        $arguments=@('build','--locked','-p','lazybuddy','-p','lazybuddy-cli','-p','lazybuddy-app-updater')
        if($Release){$arguments+='--release'}
        & cargo @arguments
        if($LASTEXITCODE){throw 'Native build failed; no old binary will be launched.'}
        foreach($file in @('module-catalog.json','app-update-source.json')){Copy-Item -LiteralPath (Join-Path $repoRoot $file) -Destination (Join-Path $target "$profile/$file") -Force}
    }
    if(!(Test-Path -LiteralPath $application)){throw 'LazyBuddy executable missing; build it first.'}
    Write-Host "Build ready: $application"
    if($BuildOnly){exit 0}
    if(Get-Process -Name lazybuddy -ErrorAction SilentlyContinue | Where-Object {$_.Path -eq $application}){
        $json='{"version":1,"command":"window.show","args":{}}'
        & $cli host $json
        if($LASTEXITCODE){throw 'Existing host did not respond. Exit it from its tray before rebuilding.'}
        exit 0
    }
    $savedPackages=$env:LAZYBUDDY_BROWSER_PACKAGES
    try{
        if($ServicePackages){$env:LAZYBUDDY_BROWSER_PACKAGES=(Resolve-Path -LiteralPath $ServicePackages).Path}else{Remove-Item Env:LAZYBUDDY_BROWSER_PACKAGES -ErrorAction SilentlyContinue}
        Start-Process -FilePath $application -WorkingDirectory $repoRoot -WindowStyle Hidden
        Write-Host 'LazyBuddy started. Optional functions are managed in the app.'
    }finally{$env:LAZYBUDDY_BROWSER_PACKAGES=$savedPackages}
}catch{Write-Host ('ERROR: '+$_.Exception.Message) -ForegroundColor Red;exit 1}
finally{Pop-Location}
