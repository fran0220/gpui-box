$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

# GitHub-hosted Windows runners have no playback device. Install a pinned,
# signed virtual endpoint so Media Foundation builds and runs a real audio
# topology instead of reducing the native media gate to compile-only coverage.
$archive = Join-Path $env:RUNNER_TEMP "Scream4.0.zip"
$destination = Join-Path $env:RUNNER_TEMP "Scream4.0"
$expectedSha256 = "fa33e25f9a46c61e4e0cd83362c51c3d2a45c6fe4091aad7507e240e40f1a520"

Invoke-WebRequest `
    -Uri "https://github.com/duncanthrax/scream/releases/download/4.0/Scream4.0.zip" `
    -OutFile $archive
$actualSha256 = (Get-FileHash -Path $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualSha256 -ne $expectedSha256) {
    throw "Scream archive checksum mismatch: $actualSha256"
}

Expand-Archive -Path $archive -DestinationPath $destination -Force
$driverDirectory = Join-Path $destination "Install\driver\x64"
$driver = Join-Path $driverDirectory "Scream.inf"
$driverBinary = Join-Path $driverDirectory "Scream.sys"
$devcon = Join-Path $destination "Install\helpers\devcon-x64.exe"

$certificate = (Get-AuthenticodeSignature -FilePath $driverBinary).SignerCertificate
if ($null -eq $certificate) {
    throw "The checksum-pinned Scream driver has no signing certificate."
}
$store = [System.Security.Cryptography.X509Certificates.X509Store]::new(
    "TrustedPublisher",
    "LocalMachine"
)
$store.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
try {
    $store.Add($certificate)
} finally {
    $store.Close()
}

& $devcon install $driver '*Scream'
if ($LASTEXITCODE -gt 1) {
    throw "Scream virtual audio device installation failed with exit code $LASTEXITCODE."
}

Set-Service -Name AudioEndpointBuilder -StartupType Manual
Set-Service -Name Audiosrv -StartupType Manual
Start-Service -Name AudioEndpointBuilder
Start-Service -Name Audiosrv

$deadline = [DateTime]::UtcNow.AddSeconds(30)
do {
    $device = Get-CimInstance Win32_SoundDevice |
        Where-Object { $_.Name -like '*Scream*' } |
        Select-Object -First 1
    if ($null -ne $device) {
        break
    }
    Start-Sleep -Seconds 1
} while ([DateTime]::UtcNow -lt $deadline)

if ($null -eq $device) {
    throw "The Scream virtual audio endpoint did not become available."
}
Write-Host "Windows native media endpoint: $($device.Name) ($($device.Status))"
