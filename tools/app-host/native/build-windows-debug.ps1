param([string]$OutputDirectory = "$PSScriptRoot/../../../target/app-host-windows")
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$out = (Resolve-Path $OutputDirectory).Path
# Use the inbox .NET Framework compiler, not PowerShell's version-dependent CLR.
$compiler = "$env:WINDIR/Microsoft.NET/Framework64/v4.0.30319/csc.exe"
if (!(Test-Path $compiler)) { throw 'Windows debug transport requires .NET Framework 4.x (x64)' }
& $compiler /nologo /optimize+ /warn:4 /warnaserror+ /platform:x64 /reference:System.Runtime.Serialization.dll "/out:$out/gpui-debug-pipe.exe" "$PSScriptRoot/windows-debug.cs"
if ($LASTEXITCODE -ne 0) { throw 'Windows debug transport build failed' }
$env:GPUI_DEBUG_PIPE_HELPER = "$out/gpui-debug-pipe.exe"
Write-Output "Built debug transport: $env:GPUI_DEBUG_PIPE_HELPER"
