# A native app-server double; never executes the argv received from the client.
$ErrorActionPreference = 'Stop'
# Explicit UTF-8 streams avoid the PowerShell host's console input buffering.
$reader = [IO.StreamReader]::new([Console]::OpenStandardInput(), [Text.UTF8Encoding]::new($false))
$writer = [IO.StreamWriter]::new([Console]::OpenStandardOutput(), [Text.UTF8Encoding]::new($false))
$writer.AutoFlush = $true
function Send($value) { $writer.WriteLine(($value | ConvertTo-Json -Depth 30 -Compress)) }
while ($null -ne ($line = $reader.ReadLine())) {
    $message = $line | ConvertFrom-Json
    switch ($message.method) {
        'initialize' { Send @{id=$message.id; result=@{}} }
        'config/read' {
            $features = @{}
            foreach ($flag in @('shell_tool','apps','plugins','hooks','browser_use','computer_use','image_generation','view_image','multi_agent','code_mode','tool_suggest','shell_snapshot','sleep_tool','skill_mcp_dependency_install','artifact')) { $features[$flag]=$false }
            Send @{id=$message.id; result=@{config=@{features=$features; web_search='disabled'; mcp_servers=@{foreign=@{enabled=$true}}}}}
        }
        { $_ -in 'thread/start','thread/resume' } {
            if ($message.params.approvalPolicy -ne 'never' -or $message.params.sandbox -ne 'read-only' -or $message.params.config.mcp_servers.foreign.enabled -ne $false) { throw 'Policy not enforced' }
            Send @{id=$message.id; result=@{thread=@{id='codex-9'}; model='codex-test'; approvalPolicy='never'; sandbox=@{type='readOnly'}}}
        }
        'mcpServerStatus/list' { Send @{id=$message.id; result=@{data=@(); nextCursor=$null}} }
        'turn/start' {
            Send @{id=$message.id; result=@{turn=@{id='turn-1'}}}
            Send @{id='tool-1'; method='item/tool/call'; params=@{threadId='codex-9'; turnId='turn-1'; callId='call-1'; tool='devcast'; namespace=$null; arguments=@{args=@('info')}}}
        }
        'command/exec' {
            if ([IO.Path]::GetFileName($message.params.command[0]) -ne 'devcast.exe' -or -not $message.params.env.DEVCAST_GROUP) { throw 'Wrong executable or group' }
            Send @{id=$message.id; result=@{exitCode=0; stdout='project info'; stderr=''}}
        }
    }
    if ($message.id -eq 'tool-1' -and $null -ne $message.result) {
        if (-not $message.result.success) { throw 'Command was rejected' }
        Send @{method='item/completed'; params=@{threadId='codex-9'; turnId='turn-1'; item=@{type='agentMessage'; text='Добавил зумы.'}}}
        $tokens = @{inputTokens=20; cachedInputTokens=10; outputTokens=5; cacheWriteInputTokens=0}
        Send @{method='thread/tokenUsage/updated'; params=@{threadId='codex-9'; turnId='turn-1'; tokenUsage=@{total=$tokens; last=$tokens}}}
        Send @{method='turn/completed'; params=@{threadId='codex-9'; turn=@{id='turn-1'; status='completed'}}}
        exit 0
    }
}
