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

# Exercise the production encoding setup against a native UTF-8 process under CP936.
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$temporary=Join-Path $root ('tmp/qa-build-encoding-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $temporary | Out-Null
$originalConsoleEncoding=[Console]::OutputEncoding
$originalOutputEncoding=$OutputEncoding
try{
    $probe=Join-Path $temporary 'utf8-json.exe'
    Add-Type -OutputAssembly $probe -OutputType ConsoleApplication -TypeDefinition @'
using System;
using System.Text;
public static class Utf8JsonFixture {
    public static void Main() {
        var bytes = Encoding.UTF8.GetBytes("{\"name\":\"B\u7ad9\",\"title\":\"\u7ea2\u679c\u77ed\u5267\",\"path\":\"bin/pythonw.exe\"}");
        Console.OpenStandardOutput().Write(bytes, 0, bytes.Length);
    }
}
'@
    [Console]::OutputEncoding=[Text.Encoding]::GetEncoding(936)
    $broken=& $probe
    $failed=$false
    try{$null=$broken | ConvertFrom-Json}catch{$failed=$true}
    if(!$failed){throw 'Expected CP936 to reproduce the malformed JSON'}
    $body=$ast.Find({param($node) $node -is [Management.Automation.Language.TryStatementAst]},$true).Body
    Invoke-Expression $body.Statements[0].Extent.Text
    Invoke-Expression $body.Statements[1].Extent.Text
    $value=(& $probe) | ConvertFrom-Json
    if($value.name -ne ('B'+[char]0x7ad9) -or $value.title -ne ([string][char]0x7ea2+[char]0x679c+[char]0x77ed+[char]0x5267) -or $value.path -ne 'bin/pythonw.exe'){throw 'UTF-8 CLI JSON was not preserved'}
    Write-Host 'PASS: CP936 reproduces the error; production UTF-8 setup preserves native JSON and Chinese text.'
}finally{
    [Console]::OutputEncoding=$originalConsoleEncoding
    $OutputEncoding=$originalOutputEncoding
    $resolved=[IO.Path]::GetFullPath($temporary)
    if(!$resolved.StartsWith((Join-Path $root 'tmp/qa-build-encoding-'),[StringComparison]::OrdinalIgnoreCase)){throw 'Unexpected test cleanup target'}
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
