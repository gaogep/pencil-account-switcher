param([int]$ParentProcessId, [string]$SetupPath, [string]$TargetPath, [string]$LogPath)
$ErrorActionPreference = 'Stop'
try {
    Wait-Process -Id $ParentProcessId -ErrorAction SilentlyContinue
    $TargetDirectory = Split-Path -LiteralPath $TargetPath
    # NSIS requires /D to be last and unquoted, including when it contains spaces.
    $Installer = Start-Process -FilePath $SetupPath -ArgumentList ('/S /UPDATE /D=' + $TargetDirectory) -PassThru -Wait
    if ($Installer.ExitCode -ne 0) { throw "Installer exited with code $($Installer.ExitCode)" }
    Start-Process -FilePath $TargetPath
} catch {
    $_.Exception.Message | Set-Content -LiteralPath $LogPath -Encoding UTF8
    if (Test-Path -LiteralPath $TargetPath) { Start-Process -FilePath $TargetPath }
} finally {
    Remove-Item -LiteralPath (Split-Path -LiteralPath $SetupPath) -Recurse -Force -ErrorAction SilentlyContinue
}
