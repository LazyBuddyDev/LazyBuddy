param([switch]$BuildOnly,[switch]$Release,[switch]$LaunchOnly,[string]$ServicePackages,[string]$ModulePackages)
$ErrorActionPreference='Stop'
if($BuildOnly -and $LaunchOnly){throw 'BuildOnly and LaunchOnly cannot be combined.'}
$repoRoot=Split-Path -Parent $PSScriptRoot
$profile=if($Release){'release'}else{'debug'}
$target=if($env:CARGO_TARGET_DIR){[IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)}else{Join-Path $repoRoot 'target'}
$application=Join-Path $target "$profile/lazybuddy.exe"
$cli=Join-Path $target "$profile/lazybuddy-cli.exe"
$candidate=$null
$developmentPackages=if($ModulePackages){(Resolve-Path -LiteralPath $ModulePackages).Path}else{Join-Path $repoRoot 'target/packages/development'}
function Invoke-Host([string]$Action){
    $ErrorActionPreference='Continue'
    $request=(@{version=1;command=$Action;args=@{}} | ConvertTo-Json -Compress).Replace('"','\"')
    $reply=& $cli host $request 2>$null
    if($LASTEXITCODE){return $null}
    return ($reply | ConvertFrom-Json)
}
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
        $running=Get-Process -Name lazybuddy -ErrorAction SilentlyContinue | Where-Object {$_.Path -eq $application}
        $arguments=@('build','--locked','-p','lazybuddy-cli','-p','lazybuddy-app-updater')
        if(!$running){$arguments+=@('-p','lazybuddy')}
        if($Release){$arguments+='--release'}
        & cargo @arguments
        if($LASTEXITCODE){throw 'Native build failed; no old binary will be launched.'}
        if($running){
            # Build alongside the running image; quit only after a new executable is ready.
            $candidate=Join-Path $target "$profile/build-and-run/lazybuddy.exe"
            New-Item -ItemType Directory -Force -Path (Split-Path -Parent $candidate) | Out-Null
            $hostArguments=@('rustc','--locked','-p','lazybuddy','--bin','lazybuddy')
            if($Release){$hostArguments+='--release'}
            $hostArguments+=@('--','-o',$candidate)
            & cargo @hostArguments
            if($LASTEXITCODE -or !(Test-Path -LiteralPath $candidate)){throw 'Host candidate build failed; running instance preserved.'}
        }
        foreach($file in @('module-catalog.json','app-update-source.json')){Copy-Item -LiteralPath (Join-Path $repoRoot $file) -Destination (Join-Path $target "$profile/$file") -Force}
    }
    if(!(Test-Path -LiteralPath $application)){throw 'LazyBuddy executable missing; build it first.'}
    Write-Host "Build ready: $application"
    if($BuildOnly){if($candidate){Write-Host "Candidate ready (running instance preserved): $candidate"};exit 0}
    $installed=& $cli modules list
    if($LASTEXITCODE){throw 'Cannot read installed modules.'}
    $installed=($installed | ConvertFrom-Json)
    $updates=@()
    if(Test-Path -LiteralPath $developmentPackages){
        foreach($module in $installed | Where-Object {$_.kind -eq 'feature'}){
            $directory=Join-Path $developmentPackages $module.id
            if(!(Test-Path -LiteralPath $directory)){continue}
            $choices=@(Get-ChildItem -LiteralPath $directory -Directory | ForEach-Object {
                $manifestPath=Join-Path $_.FullName 'module.json'
                if(Test-Path -LiteralPath $manifestPath){
                    $manifest=Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
                    if($manifest.id -ne $module.id){throw 'Development package ID mismatch.'}
                    if([version]$manifest.version -gt [version]$module.version){[pscustomobject]@{path=$_.FullName;version=[version]$manifest.version;id=$module.id}}
                }
            } | Sort-Object version -Descending)
            if($choices.Count){$updates+= $choices[0]}
        }
    }
    $status=Invoke-Host 'status'
    if($status -and $status.ok){
        if([IO.Path]::GetFullPath($status.data.exe) -ne [IO.Path]::GetFullPath($application)){throw 'Another installation is running; it will not be stopped.'}
        if(!$candidate -and !$updates.Count){
            $shown=Invoke-Host 'window.show'
            if(!$shown -or !$shown.ok){throw 'Existing host did not respond.'}
            exit 0
        }
        $hostProcess=Get-Process -Id $status.data.pid -ErrorAction Stop
        $quit=Invoke-Host 'app.quit'
        if(!$quit -or !$quit.ok -or !$hostProcess.WaitForExit(30000)){throw 'Host did not exit normally; files and modules were not replaced.'}
    }
    if($candidate){Copy-Item -LiteralPath $candidate -Destination $application -Force}
    foreach($package in $updates){
        Write-Host "Installing prepared development module: $($package.id) $($package.version)"
        & $cli modules install-local $package.path
        if($LASTEXITCODE){throw 'Development module installation failed; no stale host will be launched.'}
    }
    $savedPackages=$env:LAZYBUDDY_BROWSER_PACKAGES
    try{
        if($ServicePackages){$env:LAZYBUDDY_BROWSER_PACKAGES=(Resolve-Path -LiteralPath $ServicePackages).Path}else{Remove-Item Env:LAZYBUDDY_BROWSER_PACKAGES -ErrorAction SilentlyContinue}
        $launched=Start-Process -FilePath $application -WorkingDirectory $repoRoot -WindowStyle Hidden -PassThru
        $deadline=[DateTime]::UtcNow.AddSeconds(30)
        do{
            $ready=Invoke-Host 'status'
            if($ready -and $ready.ok -and $ready.data.pid -eq $launched.Id){break}
            $launched.Refresh()
            if($launched.HasExited -or [DateTime]::UtcNow -ge $deadline){throw 'New host did not become ready; startup failed.'}
            Start-Sleep -Milliseconds 150
        }while($true)
        Write-Host 'LazyBuddy started. Optional functions are managed in the app.'
    }finally{$env:LAZYBUDDY_BROWSER_PACKAGES=$savedPackages}
}catch{Write-Host ('ERROR: '+$_.Exception.Message) -ForegroundColor Red;exit 1}
finally{Pop-Location}
