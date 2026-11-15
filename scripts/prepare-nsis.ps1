$ErrorActionPreference='Stop'
$repoRoot=Split-Path -Parent $PSScriptRoot
$toolRoot=Join-Path $repoRoot 'target/tools/nsis'
$archive=Join-Path $toolRoot 'nsis-3.11.zip'
$compiler=Join-Path $toolRoot 'compiler/nsis-3.11/makensis.exe'
$expected='c7d27f780ddb6cffb4730138cd1591e841f4b7edb155856901cdf5f214394fa1'
New-Item -ItemType Directory -Force -Path $toolRoot | Out-Null
if(!(Test-Path -LiteralPath $archive) -or (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $expected){
    Invoke-WebRequest -Uri 'https://downloads.sourceforge.net/project/nsis/NSIS%203/3.11/nsis-3.11.zip' -OutFile $archive
}
if((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $expected){throw 'NSIS archive SHA256 mismatch'}
if(!(Test-Path -LiteralPath $compiler)){
    Expand-Archive -LiteralPath $archive -DestinationPath (Join-Path $toolRoot 'compiler')
}
Write-Output $compiler
