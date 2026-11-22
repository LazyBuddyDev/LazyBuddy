#Requires -Version 7.0
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$tmp = Join-Path $repo 'tmp'
if (!(Test-Path -LiteralPath $tmp)) { New-Item -ItemType Directory -Path $tmp | Out-Null }
if ((Get-Item -LiteralPath $tmp).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Unexpected test root.' }
$fixture = Join-Path $tmp "qa-cleanup-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $fixture | Out-Null
$junction = Join-Path $fixture 'tmp/qa-linked/points-outside'
$script = Join-Path $fixture 'scripts/cleanup.ps1'

function Assert($value, [string]$message) {
    if (!$value) { throw $message }
}

function Write-Fixture([string]$Path, [string]$Text) {
    $file = Join-Path $fixture $Path
    [IO.Directory]::CreateDirectory((Split-Path -Parent $file)) | Out-Null
    Set-Content -LiteralPath $file -Value $Text -Encoding utf8
}

function Run-Cleanup([int]$Code, [scriptblock]$Arguments) {
    $path = $script.Replace("'", "''")
    $command = "& { $Arguments } '$path'; exit `$LASTEXITCODE"
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
    $json = & ([Environment]::ProcessPath) -NoProfile -EncodedCommand $encoded
    Assert ($LASTEXITCODE -eq $Code) "Unexpected cleanup exit code: $LASTEXITCODE. Output: $json"
    return ($json | ConvertFrom-Json)
}

try {
    & git -C $fixture init --quiet
    Assert ($LASTEXITCODE -eq 0) 'Git fixture failed.'
    Write-Fixture '.gitignore' "/tmp/`n/target/"
    Write-Fixture 'notes.md' 'Keep target/packages/development/sample/1.0.1 for regression.'
    New-Item -ItemType Directory -Path (Split-Path -Parent $script) | Out-Null
    Copy-Item -LiteralPath (Join-Path $repo 'scripts/cleanup.ps1') -Destination $script
    Write-Fixture 'tmp/qa-finished/data.txt' 'generated data'
    Write-Fixture 'tmp/qa-tracked/source.txt' 'tracked source'
    & git -C $fixture add -- .gitignore notes.md
    Assert ($LASTEXITCODE -eq 0) 'Git fixture files failed.'
    & git -C $fixture add -f -- tmp/qa-tracked/source.txt
    Assert ($LASTEXITCODE -eq 0) 'Tracked file fixture failed.'
    foreach ($version in '1.0.0', '1.0.1', '1.0.2') {
        $path = "target/packages/development/sample/$version"
        Write-Fixture "$path/module.json" (@{schema=1;id='sample';version=$version;target='windows-x86_64';entrypoint='bin/service.exe'} | ConvertTo-Json -Compress)
        Write-Fixture "$path/bin/service.exe" 'mock binary'
    }
    for ($i = 1; $i -le 4; $i++) {
        $path = Join-Path $fixture "target/debug/incremental/lazybuddy-$($i.ToString('D13'))"
        Write-Fixture "target/debug/incremental/lazybuddy-$($i.ToString('D13'))/s-example/cache.bin" 'generated cache'
        $stamp = [DateTime]::UtcNow.AddDays($(if ($i -eq 2) {-1} else {-13 + $i}))
        Get-ChildItem -Force -Recurse -LiteralPath $path | ForEach-Object { $_.LastWriteTimeUtc = $stamp }
        (Get-Item -LiteralPath $path).LastWriteTimeUtc = $stamp
    }
    Write-Fixture 'tmp/link-target/keep.txt' 'keep through reparse point'
    New-Item -ItemType Directory -Path (Split-Path -Parent $junction) | Out-Null
    New-Item -ItemType Junction -Path $junction -Target (Join-Path $fixture 'tmp/link-target') | Out-Null

    $preview = Run-Cleanup 0 {param($script) & $script}
    Assert ($preview.mode -eq 'preview' -and $preview.removed_bytes -eq 0) 'Default cleanup modified files.'
    $caches = @($preview.entries | Where-Object status -EQ 'candidate')
    Assert ($caches.Count -eq 2 -and $caches[0].path -like '*0000000000003' -and $caches[1].path -like '*0000000000001') 'Cache retention is wrong.'

    $blocked = Run-Cleanup 1 {
        param($script)
        & $script -ExpiredRun @('tmp/qa-linked','tmp/qa-tracked','tmp/qa-finished/../../docs') `
            -RetiredPackage @('target/packages/development/sample/1.0.2','target/packages/development/sample/1.0.1')
    }
    $skips = @($blocked.entries | Where-Object status -EQ 'skipped')
    Assert ($skips.Count -eq 5) 'Expected rejection for link, tracked, escaped, latest and referenced paths.'

    $result = Run-Cleanup 0 {
        param($script)
        & $script -Apply -ExpiredRun 'tmp/qa-finished' -RetiredPackage 'target/packages/development/sample/1.0.0'
    }
    Assert ($result.mode -eq 'apply' -and $result.removed_bytes -gt 0) 'Apply returned no removals.'
    Assert (@($result.entries | Where-Object status -EQ 'removed').Count -eq 4) 'Wrong number of removals.'
    Assert (!(Test-Path -LiteralPath (Join-Path $fixture 'tmp/qa-finished'))) 'Finished fixture remains.'
    Assert (Test-Path -LiteralPath (Join-Path $fixture 'tmp/link-target/keep.txt')) 'Link target was modified.'
    Assert (Test-Path -LiteralPath (Join-Path $fixture 'tmp/qa-tracked/source.txt')) 'Tracked file was removed.'
    Assert (Test-Path -LiteralPath (Join-Path $fixture 'target/packages/development/sample/1.0.2/bin/service.exe')) 'Latest package was removed.'
    Assert (@((Run-Cleanup 0 {param($script) & $script}).entries).Count -eq 0) 'Second preview should be empty.'
    $absent = Run-Cleanup 0 {param($script) & $script -Apply -ExpiredRun 'tmp/qa-finished'}
    Assert ($absent.entries[0].status -eq 'missing' -and $absent.removed_bytes -eq 0) 'Reapplying cleanup should be harmless.'

    Write-Fixture 'tmp/qa-active/hold.ps1' @'
$ready = Join-Path $PSScriptRoot 'ready'
[IO.File]::WriteAllText($ready, '')
for ($i = 0; $i -lt 300; $i++) {
    if (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'done')) { exit 0 }
    Start-Sleep -Milliseconds 100
}
exit 1
'@
    $info = [Diagnostics.ProcessStartInfo]::new([Environment]::ProcessPath)
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    foreach ($arg in @('-NoProfile','-File',(Join-Path $fixture 'tmp/qa-active/hold.ps1'))) { $info.ArgumentList.Add($arg) }
    $worker = [Diagnostics.Process]::Start($info)
    try {
        $until = [DateTime]::UtcNow.AddSeconds(5)
        while (!(Test-Path -LiteralPath (Join-Path $fixture 'tmp/qa-active/ready'))) {
            if ($worker.HasExited -or [DateTime]::UtcNow -gt $until) { throw 'Test process did not become ready.' }
            Start-Sleep -Milliseconds 100
        }
        $busy = Run-Cleanup 1 {param($script) & $script -Apply -ExpiredRun 'tmp/qa-active'}
        Assert ($busy.entries[0].status -eq 'failed' -and $busy.entries[0].reason -match 'process.*using') 'Active run was not recognized.'
        Assert (!$worker.HasExited) 'Cleanup stopped the test process.'
    } finally {
        Write-Fixture 'tmp/qa-active/done' ''
        Assert ($worker.WaitForExit(5000) -and $worker.ExitCode -eq 0) 'Test process did not shut down normally.'
        $worker.Dispose()
    }
    'Workspace cleanup preview, apply, retention, references and path guards passed.'
} finally {
    $resolved = (Resolve-Path -LiteralPath $fixture).Path
    if ((Split-Path -Parent $resolved) -ne (Resolve-Path -LiteralPath $tmp).Path -or
        (Split-Path -Leaf $resolved) -notmatch '^qa-cleanup-[a-f0-9]{32}$') { throw 'Unexpected test cleanup path.' }
    if (Test-Path -LiteralPath $junction) {
        $item = Get-Item -Force -LiteralPath $junction
        Assert ($item.LinkType -eq 'Junction') 'Unexpected test link type.'
        $item.Delete()
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
