$ErrorActionPreference='Stop'
$source=Join-Path $PSScriptRoot '../build-and-run.ps1'
$parseErrors=$null
$ast=[Management.Automation.Language.Parser]::ParseFile($source,[ref]$null,[ref]$parseErrors)
if($parseErrors.Count){throw $parseErrors[0]}
$function=$ast.Find({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Stop-BuildHost'},$true)
Invoke-Expression $function.Extent.Text
$application=Join-Path $PSScriptRoot '../../target/debug/lazybuddy.exe'
$script:quitCount=0
$script:exitResult=$true
function Invoke-Host([string]$Action){
    if($Action -ne 'app.quit'){throw "Unexpected action: $Action"}
    $script:quitCount++
    return @{ok=$true}
}
function Get-Process {
    param($Id,$ErrorAction)
    $process=New-Object PSObject
    $process | Add-Member ScriptMethod WaitForExit {param($Timeout) if($Timeout -ne 30000){throw 'Unexpected timeout'};return $script:exitResult}
    return $process
}
$status=@{data=@{exe=$application;pid=123}}
Stop-BuildHost $status
if($script:quitCount -ne 1){throw 'Matching host must quit once'}
$status.data.exe=Join-Path $PSScriptRoot 'other/lazybuddy.exe'
$rejected=$false
try{Stop-BuildHost $status}catch{$rejected=$true}
if(!$rejected -or $script:quitCount -ne 1){throw 'Other installation must remain untouched'}
$status.data.exe=$application
$script:exitResult=$false
$rejected=$false
try{Stop-BuildHost $status}catch{$rejected=$true}
if(!$rejected){throw 'Exit timeout must stop the build'}
Write-Host 'PASS: matching host exits normally, other installs are preserved, exit timeout fails safely.'
