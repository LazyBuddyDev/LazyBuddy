#Requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$Apply,
    [string[]]$ExpiredRun = @(),
    [string[]]$RetiredPackage = @()
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$prefix = $root + [IO.Path]::DirectorySeparatorChar
$comparison = [StringComparison]::OrdinalIgnoreCase
$cutoff = [DateTime]::UtcNow.AddDays(-7)
$versionPattern = '(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)'
$temporaryPattern = '(?:tmp/qa-[a-z0-9][a-z0-9-]*|tmp/profiles/[a-z0-9][a-z0-9-]*)'

function Get-SafeItem([string]$RelativePath) {
    $path = [IO.Path]::GetFullPath((Join-Path $root $RelativePath))
    if (!$path.StartsWith($prefix, $comparison)) { throw 'Path escapes the workspace.' }
    $parent = $root
    foreach ($part in $path.Substring($prefix.Length).Split([IO.Path]::DirectorySeparatorChar)) {
        $parent = Join-Path $parent $part
        try { $item = Get-Item -Force -LiteralPath $parent }
        catch [System.Management.Automation.ItemNotFoundException] { return }
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw 'Path contains a reparse point.'
        }
    }
    return $item
}

function Measure-DisposableDirectory([IO.DirectoryInfo]$Directory) {
    $pending = [Collections.Generic.Stack[IO.DirectoryInfo]]::new()
    $pending.Push($Directory)
    $bytes = 0L
    $modified = $Directory.LastWriteTimeUtc
    while ($pending.Count) {
        foreach ($item in $pending.Pop().EnumerateFileSystemInfos()) {
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw 'Tree contains a reparse point.'
            }
            if ($item.Name -match '^(?:\.git|\.env(?:\..*)?|credentials?(?:\..*)?|tokens?(?:\..*)?)$' -or
                $item.Extension -in '.pfx', '.p12', '.pvk', '.dpapi') {
                throw 'Tree contains source control or possible credentials.'
            }
            if ($item.LastWriteTimeUtc -gt $modified) { $modified = $item.LastWriteTimeUtc }
            if ($item -is [IO.DirectoryInfo]) { $pending.Push($item) }
            else { $bytes += $item.Length }
        }
    }
    return @{Bytes = $bytes; Modified = $modified}
}

function Assert-Ignored([string]$RelativePath) {
    $tracked = @(& git -C $root --literal-pathspecs ls-files -- $RelativePath)
    if ($LASTEXITCODE -ne 0 -or $tracked.Count) { throw 'Git check failed or path contains tracked files.' }
    & git -C $root check-ignore --quiet --no-index -- $RelativePath
    if ($LASTEXITCODE -ne 0) { throw 'Path is outside the ignored build and test output.' }
}

function Read-FeaturePackage([string]$RelativePath) {
    if ($RelativePath -cnotmatch "^target/packages/development/([a-z][a-z0-9-]{0,63})/($versionPattern)$") {
        throw 'Expected a version directory in target/packages/development.'
    }
    $id = $Matches[1]
    $version = $Matches[2]
    foreach ($part in $version.Split('.')) { [void][uint32]::Parse($part) }
    $manifestFile = Get-SafeItem "$RelativePath/module.json"
    if ($manifestFile -isnot [IO.FileInfo] -or $manifestFile.Length -gt 65536) {
        throw 'Missing or oversized module manifest.'
    }
    $manifest = Get-Content -LiteralPath $manifestFile.FullName -Raw -Encoding utf8 | ConvertFrom-Json
    if ($manifest.schema -ne 1 -or $manifest.id -cne $id -or $manifest.version -cne $version -or
        $manifest.kind -notin $null, 'feature' -or $manifest.target -cne 'windows-x86_64' -or
        $manifest.entrypoint -cnotmatch '^(?:[A-Za-z0-9_-][A-Za-z0-9_.-]*/)*[A-Za-z0-9_-][A-Za-z0-9_.-]*$' -or
        (Get-SafeItem "$RelativePath/$($manifest.entrypoint)") -isnot [IO.FileInfo]) {
        throw 'Directory does not contain a complete staged Windows feature package.'
    }
    return @{Id = $id; Version = $version.Split('.') | ForEach-Object { [uint32]$_ }}
}

function Assert-RetiredPackage([string]$RelativePath) {
    $package = Read-FeaturePackage $RelativePath
    $parent = Get-SafeItem "target/packages/development/$($package.Id)"
    $newer = $false
    foreach ($sibling in $parent.EnumerateDirectories()) {
        if ($sibling.Name -cnotmatch "^$versionPattern$" -or $sibling.Attributes -band [IO.FileAttributes]::ReparsePoint) { continue }
        try { $version = (Read-FeaturePackage "target/packages/development/$($package.Id)/$($sibling.Name)").Version }
        catch { continue }
        for ($i = 0; $i -lt 3; $i++) {
            if ($version[$i] -eq $package.Version[$i]) { continue }
            if ($version[$i] -gt $package.Version[$i]) { $newer = $true }
            break
        }
        if ($newer) { break }
    }
    if (!$newer) { throw 'No newer complete staged feature package exists.' }
}

function Assert-Unreferenced([string]$RelativePath) {
    $references = @(& git -C $root grep -l -I -i -F -e $RelativePath -e $RelativePath.Replace('/', '\') --)
    if ($LASTEXITCODE -eq 0) { throw "Referenced by tracked files: $($references -join ', ')" }
    if ($LASTEXITCODE -ne 1) { throw 'Git reference check failed.' }
}

function New-Candidate([string]$Path, [string]$Kind) {
    $entry = [ordered]@{path = $Path; kind = $Kind; status = 'candidate'; bytes = 0L; reason = $null}
    $modified = [DateTime]::MinValue
    $itemName = ''
    try {
        if ($Kind -eq 'expired_run' -and $Path -cnotmatch "^$temporaryPattern$") {
            throw 'Expected one named QA directory under tmp or tmp/profiles.'
        }
        if ($Kind -eq 'retired_package' -and $Path -cnotmatch "^target/packages/development/[a-z][a-z0-9-]{0,63}/$versionPattern$") {
            throw 'Expected one staged feature version directory.'
        }
        $item = Get-SafeItem $Path
        if (!$item) { $entry.status = 'missing' }
        else {
            if ($item -isnot [IO.DirectoryInfo]) { throw 'Path is not a directory.' }
            Assert-Ignored $Path
            if ($Kind -ne 'incremental') { Assert-Unreferenced $Path }
            if ($Kind -eq 'retired_package') { Assert-RetiredPackage $Path }
            if ($Kind -eq 'incremental') {
                $children = @($item.EnumerateFileSystemInfos())
                if (!$children.Count -or @($children | Where-Object { $_.Name -cnotmatch '^s-[a-z0-9-]+(?:\.lock)?$' }).Count) {
                    throw 'Directory does not have the expected Cargo incremental layout.'
                }
            }
            $measurement = Measure-DisposableDirectory $item
            $entry.bytes = $measurement.Bytes
            $modified = $measurement.Modified
            $itemName = $item.Name
        }
    } catch {
        $entry.status = 'skipped'
        $entry.reason = $_.Exception.Message
    }
    return [pscustomobject]@{Entry = [pscustomobject]$entry; Modified = $modified; Name = $itemName}
}

function Get-ActiveProcesses {
    $processes = @(Get-CimInstance -ClassName Win32_Process -Property Name,ProcessId,ExecutablePath,CommandLine)
    if (!($processes | Where-Object ProcessId -EQ $PID)) { throw 'Cannot verify the Windows process list.' }
    return $processes
}

function Test-Occupied($Entry, $Processes) {
    $absolute = (Join-Path $root $Entry.path).Replace('/', '\')
    $busy = @($Processes | Where-Object {
        ($Entry.kind -ne 'expired_run' -and $_.Name -match '^(cargo|rustc|link)\.exe$') -or
        ($_.ExecutablePath -and $_.ExecutablePath.StartsWith($absolute + '\', $comparison)) -or
        ($_.CommandLine -and $_.CommandLine.IndexOf($absolute, $comparison) -ge 0)
    })
    return $busy.Count -gt 0
}

if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw 'The cleanup script requires Windows.' }
for ($ancestor = Get-Item -Force -LiteralPath $root; $ancestor; $ancestor = $ancestor.Parent) {
    if ($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Workspace is under a reparse point.' }
}
$gitRoot = & git -C $root rev-parse --show-toplevel
if ($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($gitRoot) -ine $root) { throw 'Expected the script inside its own Git workspace.' }

$report = @()
foreach ($profile in 'debug', 'release') {
    $relative = "target/$profile/incremental"
    try { $directory = Get-SafeItem $relative }
    catch {
        $report += [pscustomobject]@{path=$relative; kind='incremental'; status='skipped'; bytes=0L; reason=$_.Exception.Message}
        continue
    }
    if ($directory -isnot [IO.DirectoryInfo]) { continue }
    $scanned = @($directory.EnumerateDirectories() | Where-Object Name -CMatch '^[a-z0-9_]+-[a-z0-9]{13}$' | ForEach-Object {
        New-Candidate "$relative/$($_.Name)" 'incremental'
    })
    $report += @($scanned | Where-Object { $_.Entry.status -ne 'candidate' } | ForEach-Object Entry)
    foreach ($group in ($scanned | Where-Object { $_.Entry.status -eq 'candidate' } | Group-Object { $_.Name.Split('-')[0] })) {
        $report += @($group.Group | Sort-Object @{Expression='Modified';Descending=$true},Name |
            Select-Object -Skip 2 | Where-Object Modified -LT $cutoff | ForEach-Object Entry)
    }
}
foreach ($path in ($ExpiredRun | Sort-Object -Unique)) {
    if ($path) { $report += (New-Candidate ($path.Replace('\', '/')) 'expired_run').Entry }
}
foreach ($path in ($RetiredPackage | Sort-Object -Unique)) {
    if ($path) { $report += (New-Candidate ($path.Replace('\', '/')) 'retired_package').Entry }
}
$candidates = @($report | Where-Object status -EQ 'candidate')
$candidateBytes = [long](($candidates | Measure-Object bytes -Sum).Sum)
if ($Apply) {
    # Recheck candidates before removal. The script never stops another process.
    foreach ($entry in $candidates) {
        try {
            $current = New-Candidate $entry.path $entry.kind
            if ($current.Entry.status -eq 'missing') { $entry.status = 'missing'; continue }
            if ($current.Entry.status -ne 'candidate') { throw $current.Entry.reason }
            if ($entry.kind -eq 'incremental' -and $current.Modified -ge $cutoff) { throw 'Cache was used after the scan.' }
            if (Test-Occupied $entry @(Get-ActiveProcesses)) { throw 'A build or process is using this path.' }
            $item = Get-SafeItem $entry.path
            Remove-Item -LiteralPath $item.FullName -Recurse -Force
            $entry.bytes = $current.Entry.bytes
            $entry.status = 'removed'
        } catch {
            $entry.status = 'failed'
            $entry.reason = $_.Exception.Message
        }
    }
}
$removed = @($report | Where-Object status -EQ 'removed')
[pscustomobject]@{
    mode = if ($Apply) {'apply'} else {'preview'}
    entries = $report
    candidate_bytes = $candidateBytes
    removed_bytes = [long](($removed | Measure-Object bytes -Sum).Sum)
} | ConvertTo-Json -Depth 4
if (@($report | Where-Object { $_.status -eq 'failed' -or ($_.status -eq 'skipped' -and ($Apply -or $_.kind -ne 'incremental')) }).Count) {
    exit 1
}
exit 0
