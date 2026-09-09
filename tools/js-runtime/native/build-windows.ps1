param([string]$OutputDirectory = "$PSScriptRoot/../../../target/js-runtime-windows")
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$out = (Resolve-Path $OutputDirectory).Path
$source = "$PSScriptRoot/windows-launch.c"

foreach ($probe in @($false, $true)) {
    $name = if ($probe) { 'windows-probe' } else { 'gpui-sandbox-launch' }
    if (Get-Command cl.exe -ErrorAction SilentlyContinue) {
        $flags = @('/nologo', '/std:c11', '/O2', '/MT', '/W4', '/D_CRT_SECURE_NO_WARNINGS')
        if ($probe) { $flags += '/DGPUI_SANDBOX_PROBE' }
        & cl.exe @flags $source "/Fo:$out/$name.obj" "/Fe:$out/$name.exe" /link userenv.lib advapi32.lib rpcrt4.lib ws2_32.lib ole32.lib shell32.lib uuid.lib
    } elseif (Get-Command gcc.exe -ErrorAction SilentlyContinue) {
        $flags = @('-std=c11', '-O2', '-Wall', '-Wextra', '-Werror', '-municode')
        if ($probe) { $flags += '-DGPUI_SANDBOX_PROBE' }
        & gcc.exe @flags $source -o "$out/$name.exe" -luserenv -ladvapi32 -lrpcrt4 -lws2_32 -lole32 -lshell32 -luuid
    } else {
        throw 'Use a Visual Studio 2022 developer shell or add MinGW-w64 GCC to PATH'
    }
    if ($LASTEXITCODE -ne 0) { throw "Windows sandbox build failed: $name" }
}
$env:GPUI_SANDBOX_LAUNCHER = "$out/gpui-sandbox-launch.exe"
$env:GPUI_WINDOWS_SANDBOX_PROBE = "$out/windows-probe.exe"
Write-Output "Built launcher: $env:GPUI_SANDBOX_LAUNCHER"
Write-Output "Built native probe: $env:GPUI_WINDOWS_SANDBOX_PROBE"
