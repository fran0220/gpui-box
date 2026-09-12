param([Parameter(Mandatory=$true)][string]$EventsPath,
      [Parameter(Mandatory=$true)][string]$FiltersPath,
      [switch]$ReadOnlyFixture)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
if (!$ReadOnlyFixture) {
    # Read existing diagnostics only. Never change collection, firewall rules,
    # capabilities or exemptions, and never stop the workflow's ETW capture.
    & netsh wfp show netevents "file=$EventsPath" protocol=6 localaddr=127.0.0.1 remoteaddr=127.0.0.1 timewindow=60 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'WFP event read failed' }
    & netsh wfp show filters "file=$FiltersPath" verbose=on | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'WFP filter read failed' }
}
[xml]$events = [System.IO.File]::ReadAllText($EventsPath)
[xml]$filters = [System.IO.File]::ReadAllText($FiltersPath)
$records = @($events.SelectNodes('//*[header]') | ForEach-Object {
    $h = $_.header
    $drop = $_.classifyDrop
    [ordered]@{
        time = [string]$h.timeStamp; protocol = [string]$h.ipProtocol
        localAddress = [string]$h.localAddrV4; remoteAddress = [string]$h.remoteAddrV4
        localPort = [string]$h.localPort; remotePort = [string]$h.remotePort
        appId = [string]$h.appId.data; sid = [string]$h.packageSid
        pid = [string]$_.internalFields.processId
        allow = $null -ne $_.classifyAllow; drop = $null -ne $drop
        filterId = [string]$drop.filterId; direction = [string]$drop.msFwpDirection
        loopback = [string]$drop.isLoopback
    }
})
$rules = @($filters.SelectNodes('//*[filterId and subLayerKey and action]') | ForEach-Object {
    [ordered]@{
        id = [string]$_.filterId; layer = [string]$_.layerKey
        sublayer = [string]$_.subLayerKey; action = [string]$_.action.type
    }
})
@{ events = $records; filters = $rules } | ConvertTo-Json -Depth 6 -Compress
