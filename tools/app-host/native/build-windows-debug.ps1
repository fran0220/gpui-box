param([string]$OutputDirectory = "$PSScriptRoot/../../../target/app-host-windows")
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$out = (Resolve-Path $OutputDirectory).Path
# Use the inbox .NET Framework compiler, not PowerShell's version-dependent CLR.
$compiler = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
if (!(Test-Path $compiler)) { throw 'Windows debug transport requires .NET Framework 4.x (x64)' }
$source = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'windows-debug.cs')).Path
$executable = Join-Path $out 'gpui-debug-pipe.exe'
# The inbox compiler must receive canonical native paths, not mixed separators.
Write-Output "Compiling debug source: $source"
& $compiler /nologo /optimize+ /warn:4 /warnaserror+ /platform:x64 /reference:System.Runtime.Serialization.dll "/out:$executable" $source
if ($LASTEXITCODE -ne 0) { throw 'Windows debug transport build failed' }
if (!(Test-Path -LiteralPath $executable -PathType Leaf)) { throw 'Windows debug compiler produced no helper' }
$env:GPUI_DEBUG_PIPE_HELPER = $executable
Write-Output "Built debug transport: $env:GPUI_DEBUG_PIPE_HELPER"
