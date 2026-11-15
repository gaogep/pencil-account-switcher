# Exercises the production helper with fixture executables; never installs Switchboard.
$ErrorActionPreference = 'Stop'
$Root = Join-Path ([IO.Path]::GetTempPath()) ('Switchboard updater fixture ' + [guid]::NewGuid())
New-Item -ItemType Directory -Path $Root | Out-Null
$env:SWITCHBOARD_UPDATE_FIXTURE = $Root
try {
    $Source = @'
using System;
using System.IO;
public class UpdateFixture {
    public static int Main(string[] args) {
        string root = Environment.GetEnvironmentVariable("SWITCHBOARD_UPDATE_FIXTURE");
        if (Path.GetFileNameWithoutExtension(Environment.GetCommandLineArgs()[0]) == "setup") {
            File.WriteAllLines(Path.Combine(root, "arguments.txt"), args);
            return Environment.GetEnvironmentVariable("SWITCHBOARD_UPDATE_FAIL") == "1" ? 42 : 0;
        }
        File.WriteAllText(Path.Combine(root, "restarted.txt"), "started");
        return 0;
    }
}
'@
    $Target = Join-Path $Root 'Installed app.exe'
    Add-Type -TypeDefinition $Source -OutputAssembly $Target -OutputType ConsoleApplication
    $Helper = Join-Path $PSScriptRoot '../crates/switchboard-platform/src/update/apply-update.ps1'
    foreach ($Fail in @('0', '1')) {
        $env:SWITCHBOARD_UPDATE_FAIL = $Fail
        $Stage = Join-Path $Root ('stage ' + $Fail)
        New-Item -ItemType Directory $Stage | Out-Null
        $Setup = Join-Path $Stage 'setup.exe'
        Copy-Item $Target $Setup
        $Log = Join-Path $Root 'error.txt'
        Remove-Item (Join-Path $Root 'restarted.txt') -ErrorAction SilentlyContinue
        $Parent = Start-Process powershell.exe -ArgumentList '-NoProfile -NonInteractive -Command "Start-Sleep -Seconds 2"' -PassThru -WindowStyle Hidden
        $Start = Get-Date
        & $Helper -ParentProcessId $Parent.Id -SetupPath $Setup -TargetPath $Target -LogPath $Log
        if (((Get-Date) - $Start).TotalSeconds -lt 1) { throw 'Did not wait for parent exit' }
        $Arguments = Get-Content (Join-Path $Root 'arguments.txt')
        if (($Arguments -join ' ') -ne ('/S /UPDATE /D=' + $Root)) { throw "Wrong NSIS arguments: $Arguments" }
        $Deadline = (Get-Date).AddSeconds(10)
        while (!(Test-Path (Join-Path $Root 'restarted.txt')) -and (Get-Date) -lt $Deadline) { Start-Sleep -Milliseconds 50 }
        if (!(Test-Path (Join-Path $Root 'restarted.txt'))) { throw 'App did not restart' }
        if (Test-Path $Stage) { throw 'Staging directory leaked' }
        if ($Fail -eq '1' -and (!(Test-Path $Log) -or !(Get-Content $Log -Raw).Contains('42'))) { throw 'Installer failure was not reported' }
        if ($Fail -eq '0' -and (Test-Path $Log)) { throw 'Successful install reported an error' }
    }
    Write-Output 'Windows updater helper: parent wait, paths with spaces, silent flags, restart, failure log and cleanup passed'
} finally {
    Remove-Item Env:SWITCHBOARD_UPDATE_FIXTURE -ErrorAction SilentlyContinue
    Remove-Item Env:SWITCHBOARD_UPDATE_FAIL -ErrorAction SilentlyContinue
    Remove-Item $Root -Recurse -Force -ErrorAction SilentlyContinue
}
