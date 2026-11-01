# Read-only client for the debug editor's bounded contract snapshot tool.
# Never starts FM, the editor, or a write tool. No save-file parsing.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateRange(1, 4294967295)]
    [uint32]$Uid,
    [ValidateRange(0, 65535)]
    [int]$Port = 0,
    [uint32]$GamePid = 0
)

$ErrorActionPreference = 'Stop'
if ($Port -eq 0) {
    $portFile = Join-Path (Split-Path $PSScriptRoot -Parent) '.mcp-port'
    if (-not (Test-Path -LiteralPath $portFile)) {
        throw 'Specify -Port with the running debug editor port; .mcp.json is only an example.'
    }
    $Port = [int](Get-Content -LiteralPath $portFile -Raw).Trim()
    if ($Port -lt 1 -or $Port -gt 65535) { throw 'Invalid .mcp-port' }
}
$endpoint = "http://127.0.0.1:$Port/mcp"
$script:contractRpcId = 0
function Invoke-ContractRpc($Method, $Parameters) {
    $script:contractRpcId++
    $body = @{
        jsonrpc = '2.0'; id = $script:contractRpcId
        method = $Method; params = $Parameters
    } | ConvertTo-Json -Depth 12 -Compress
    $reply = Invoke-RestMethod -Uri $endpoint -Method Post -ContentType 'application/json' `
        -Headers @{Accept = 'application/json, text/event-stream'} -Body $body -TimeoutSec 30
    if ($reply.error) { throw $reply.error.message }
    if ($null -eq $reply.result) { throw 'Missing MCP result' }
    return $reply.result
}
function Invoke-ContractReadTool($Name, $Arguments) {
    $reply = Invoke-ContractRpc 'tools/call' @{name = $Name; arguments = $Arguments}
    if ($reply.isError) { throw ($reply.content | ConvertTo-Json -Depth 8 -Compress) }
    $text = @($reply.content | Where-Object type -eq 'text')
    if ($text.Count -ne 1) { throw 'Expected one JSON text result' }
    return ($text[0].text | ConvertFrom-Json)
}

$null = Invoke-ContractRpc 'initialize' @{
    protocolVersion = '2025-06-18'; capabilities = @{}
    clientInfo = @{name = 'fm-editor-contract-inspection'; version = '1'}
}
$null = Invoke-RestMethod -Uri $endpoint -Method Post -ContentType 'application/json' `
    -Body '{"jsonrpc":"2.0","method":"notifications/initialized"}' -TimeoutSec 30
$discovery = Invoke-ContractRpc 'tools/list' @{}
foreach ($name in @('attach_status', 'search_entities', 'snapshot_player_contract')) {
    if (@($discovery.tools | Where-Object name -eq $name).Count -ne 1) {
        throw "Required read-only tool unavailable: $name. Rebuild the debug editor."
    }
}
$snapshotSchema = ($discovery.tools | Where-Object name -eq 'snapshot_player_contract').inputSchema
$required = @($snapshotSchema.required | Sort-Object)
if (($required -join ',') -ne 'expected_address,expected_uid,index,pid') {
    throw 'Contract snapshot schema changed; inspect tools/list before updating this client.'
}
$attachArguments = @{}
if ($GamePid -ne 0) { $attachArguments.pid = $GamePid }
$status = Invoke-ContractReadTool 'attach_status' $attachArguments
if (-not $status.attached -or $status.players -le 0 -or
    @($status.running_games | Where-Object pid -eq $status.pid).Count -ne 1) {
    throw 'FM is not running with a readable player array. No snapshot taken.'
}
$entities = @(Invoke-ContractReadTool 'search_entities' @{
    entity_type = 'player'; query = $Uid.ToString(); rebuild_cache = $true
} | Where-Object uid -eq $Uid)
if ($entities.Count -ne 1) { throw "UID $Uid is missing or ambiguous in fresh discovery" }
$entity = $entities[0]
$snapshot = Invoke-ContractReadTool 'snapshot_player_contract' @{
    pid = $status.pid; index = $entity.index
    expected_address = $entity.address; expected_uid = $Uid
}
@{
    endpoint = $endpoint
    captured_utc = [DateTime]::UtcNow.ToString('o')
    discovery_name = $entity.name
    tool_schema = $snapshotSchema
    snapshot = $snapshot
} | ConvertTo-Json -Depth 15
