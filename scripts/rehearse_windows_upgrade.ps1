param([string]$PreviousInstaller, [string]$NextInstaller, [string]$NextVersion)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Use an isolated CI runner for installer rehearsals' }
foreach ($Package in @($PreviousInstaller, $NextInstaller)) {
    node (Join-Path $PSScriptRoot 'release_assets.mjs') verify-package $Package
    if ($LASTEXITCODE -ne 0) { throw 'Package signature verification failed; installation was not started' }
}
$Root = Join-Path ([IO.Path]::GetTempPath()) ('Switchboard upgrade rehearsal ' + [guid]::NewGuid())
$Install = Join-Path $Root 'Installed application'
$SavedLocal = $env:LOCALAPPDATA
$SavedDemo = $env:SWITCHBOARD_DEMO
$SavedAutoOpen = $env:SWITCHBOARD_AUTO_OPEN
$env:LOCALAPPDATA = Join-Path $Root 'LocalAppData'
$env:SWITCHBOARD_DEMO = '1'
$env:SWITCHBOARD_AUTO_OPEN = '1'
$Data = Join-Path $env:LOCALAPPDATA 'Switchboard'
$Target = Join-Path $Install 'switchboard-app.exe'
$Report = Join-Path $PSScriptRoot '../build/rehearsal/windows-report.json'
try {
    New-Item -ItemType Directory -Force (Join-Path $Data 'credentials') | Out-Null
    $First = Start-Process $PreviousInstaller -ArgumentList ('/S /D=' + $Install) -Wait -PassThru
    if ($First.ExitCode -ne 0 -or !(Test-Path $Target)) { throw 'Previous installer failed' }
    $Accounts = 1..3 | ForEach-Object {
        $Id = '{0:00000000}-1111-4111-8111-111111111111' -f $_
        $Email = "fixture$_@example.invalid"
        [IO.File]::WriteAllText((Join-Path $Data "credentials/$Id.json"), (@{ email = $Email; token = 'fixture-only' } | ConvertTo-Json -Compress))
        @{ id = $Id; email = $Email; isCurrent = ($_ -eq 1); createdAt = 0; authState = 'valid' }
    }
    [IO.File]::WriteAllText((Join-Path $Data 'store.json'), (@{ storeVersion = 1; accounts = @($Accounts) } | ConvertTo-Json -Depth 5))
    $Before = @{}
    Get-ChildItem $Data -Recurse -File | ForEach-Object { $Before[$_.FullName] = (Get-FileHash $_.FullName -Algorithm SHA256).Hash }
    $OldHash = (Get-FileHash $Target -Algorithm SHA256).Hash
    $Stage = Join-Path $Root 'verified update staging'
    New-Item -ItemType Directory $Stage | Out-Null
    $Setup = Join-Path $Stage 'setup.exe'
    Copy-Item $NextInstaller $Setup
    $Log = Join-Path $Data 'update-error.txt'
    # The production helper must wait for its parent and then invoke real NSIS.
    $Parent = Start-Process powershell.exe -ArgumentList '-NoProfile -NonInteractive -Command "Start-Sleep -Seconds 2"' -PassThru -WindowStyle Hidden
    & (Join-Path $PSScriptRoot '../crates/switchboard-platform/src/update/apply-update.ps1') -ParentProcessId $Parent.Id -SetupPath $Setup -TargetPath $Target -LogPath $Log
    if (Test-Path $Log) { throw (Get-Content $Log -Raw) }
    $Deadline = (Get-Date).AddSeconds(30)
    $App = $null
    while ((Get-Date) -lt $Deadline) {
        $App = Get-Process -Name 'switchboard-app' -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Target } | Select-Object -First 1
        if ($App) { break }
        Start-Sleep -Milliseconds 100
    }
    if (!$App) { throw 'Updated app did not restart' }
    $ActualVersion = [Diagnostics.FileVersionInfo]::GetVersionInfo($Target).ProductVersion
    if ($ActualVersion -notmatch ('^' + [regex]::Escape($NextVersion) + '(\.0)?$')) { throw "Wrong installed version: $ActualVersion" }
    if ((Get-FileHash $Target -Algorithm SHA256).Hash -eq $OldHash) { throw 'Executable was not replaced' }
    foreach ($Path in $Before.Keys) {
        if ((Get-FileHash $Path -Algorithm SHA256).Hash -ne $Before[$Path]) { throw 'Protected account fixture changed' }
    }
    if (Test-Path $Stage) { throw 'Update staging was not cleaned' }
    New-Item -ItemType Directory -Force (Split-Path $Report) | Out-Null
    @{ installedVersion = $ActualVersion; accountFixtures = 3; protectedFiles = $Before.Count; protectedBytesUnchanged = $true; restarted = $true; executableReplaced = $true; realLoginTested = $false; startupMigrationTested = $false } | ConvertTo-Json | Set-Content $Report -Encoding UTF8
    Write-Output 'Windows signed NSIS upgrade, app restart and three-account fixture preservation passed'
} finally {
    # Only this disposable installation's app processes are eligible for cleanup.
    Get-Process -Name 'switchboard-app' -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Target } | Stop-Process -Force
    $Uninstall = Join-Path $Install 'uninstall.exe'
    if (Test-Path $Uninstall) { Start-Process $Uninstall -ArgumentList '/S' -Wait }
    $env:LOCALAPPDATA = $SavedLocal
    $env:SWITCHBOARD_DEMO = $SavedDemo
    $env:SWITCHBOARD_AUTO_OPEN = $SavedAutoOpen
    Remove-Item $Root -Recurse -Force -ErrorAction SilentlyContinue
}
