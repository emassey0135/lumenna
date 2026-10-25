# The module's one `lum rpc` per profile: started at the first request, started again if it
# stopped, and stopped with the module. Requests and replies are newline-delimited JSON-RPC on its
# standard input and output, as UTF-8 bytes whatever the console's code page. What it pushes
# unasked (`lumenna/changed`) is passed over. When the Lumenna app or `lum`'s daemon is running,
# `lum rpc` relays to it, so a write goes through whatever holds the store.
#
# Dot-sourced by the module and by Write-Operations.ps1. Windows PowerShell 5.1 and PowerShell 7.

# Each running server, by the profile it serves ('' for the platform's own), and the lock every
# request takes, so two threads using this module never interleave on one pipe.
$script:Servers = @{}
$script:ServersLock = New-Object System.Object
$script:NextRequest = 0

# Where `lum` is: on PATH, else beside the module, else where the Windows installer puts it.
function Find-Lum {
    # Indexed, not `Select-Object -First 1`, whose stopping the pipeline Windows PowerShell 5.1
    # leaves in a caller's -ErrorVariable.
    $onPath = @(Get-Command -Name lum -CommandType Application -ErrorAction Ignore)
    if ($onPath.Count) { return $onPath[0].Path }
    foreach ($name in 'lum.exe', 'lum') {
        $beside = Join-Path $PSScriptRoot $name
        if (Test-Path -LiteralPath $beside -PathType Leaf) { return $beside }
    }
    if ($env:LOCALAPPDATA) {
        $installed = Join-Path $env:LOCALAPPDATA 'Programs\Lumenna\lum.exe'
        if (Test-Path -LiteralPath $installed -PathType Leaf) { return $installed }
    }
    $null
}

# Starts `lum rpc` for a profile; $null when there is no `lum` to start.
function Start-LumennaServer([string] $ProfileDirectory) {
    $lum = Find-Lum
    if (-not $lum) { return $null }
    $utf8 = New-Object System.Text.UTF8Encoding $false
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = $lum
    $start.Arguments = 'rpc'
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.StandardOutputEncoding = $utf8
    $start.StandardErrorEncoding = $utf8
    $start.EnvironmentVariables['NO_COLOR'] = '1'
    if ($ProfileDirectory) { $start.EnvironmentVariables['LUMENNA_PROFILE'] = $ProfileDirectory }
    $process = [System.Diagnostics.Process]::Start($start)
    @{
        Process = $process
        Input   = $process.StandardInput.BaseStream
        Output  = $process.StandardOutput
        # Drained as it comes, so a server with much to say there never blocks on it; read for
        # why, should it stop.
        Errors  = $process.StandardError.ReadToEndAsync()
    }
}

# Stops one: the end of its input is the end of its work.
function Stop-LumennaServer($Server) {
    try { $Server.Input.Close() } catch { }
    try {
        if (-not $Server.Process.WaitForExit(5000)) { $Server.Process.Kill() }
    } catch { }
    $Server.Process.Dispose()
}

# Stops every one: when the module is removed, and between tests.
function Stop-LumennaServers {
    [System.Threading.Monitor]::Enter($script:ServersLock)
    try {
        foreach ($server in @($script:Servers.Values)) { Stop-LumennaServer $server }
        $script:Servers.Clear()
    } finally {
        [System.Threading.Monitor]::Exit($script:ServersLock)
    }
}

# Sends one request to the profile's server, starting it if it is not running, and returns its
# reply: an object with `result`, or `error` and its `message`. $null when there is no `lum`.
function Invoke-LumennaRpc {
    param(
        [string] $Method,
        $Params = @{},
        # The profile, else LUMENNA_PROFILE's, else the platform's own: a server per profile.
        [string] $ProfileDirectory
    )
    $key = if ($ProfileDirectory) { $ProfileDirectory } elseif ($env:LUMENNA_PROFILE) { $env:LUMENNA_PROFILE } else { '' }
    [System.Threading.Monitor]::Enter($script:ServersLock)
    try {
        $script:NextRequest++
        $id = $script:NextRequest
        $request = ConvertTo-Json -InputObject ([ordered] @{ jsonrpc = '2.0'; id = $id; method = $Method; params = $Params }) -Depth 64 -Compress
        $bytes = (New-Object System.Text.UTF8Encoding $false).GetBytes($request + "`n")
        # A server that stopped since the last request is started again, as is one whose pipe
        # breaks as this is written, since it cannot have taken the request in.
        $server = $null
        foreach ($attempt in 1, 2) {
            $server = $script:Servers[$key]
            if ($server -and $server.Process.HasExited) {
                Stop-LumennaServer $server
                $script:Servers.Remove($key)
                $server = $null
            }
            if (-not $server) {
                $server = Start-LumennaServer $key
                if (-not $server) { return $null }
                $script:Servers[$key] = $server
            }
            try {
                $server.Input.Write($bytes, 0, $bytes.Length)
                $server.Input.Flush()
                break
            } catch [System.IO.IOException] {
                Stop-LumennaServer $server
                $script:Servers.Remove($key)
                if ($attempt -eq 2) { throw }
            }
        }
        while ($true) {
            $line = $server.Output.ReadLine()
            if ($null -eq $line) {
                # It stopped with the request in hand: whether that was done is not known, so it
                # is not sent again.
                $server.Process.WaitForExit()
                $said = ($server.Errors.Result -split '\r?\n' | Where-Object { $_ }) -join ' '
                $why = if ($said) { "lum rpc stopped: $said" } else { "lum rpc stopped, ending with $($server.Process.ExitCode)" }
                Stop-LumennaServer $server
                $script:Servers.Remove($key)
                return [pscustomobject] @{ error = [pscustomobject] @{ message = $why } }
            }
            if (-not $line.Trim()) { continue }
            $reply = $line | ConvertFrom-Json
            # Anything else is pushed unasked, such as lumenna/changed: nothing here waits for it.
            if ($reply.PSObject.Properties['id'] -and $reply.id -eq $id) { return $reply }
        }
    } finally {
        [System.Threading.Monitor]::Exit($script:ServersLock)
    }
}
