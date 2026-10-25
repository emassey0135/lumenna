# Lumenna in PowerShell: every operation an assistant can do (the core's assistant table) as a
# cmdlet, over one `lum rpc` for the module's session (Lumenna.Rpc.ps1). The cmdlets themselves
# are written from the table into Lumenna.Operations.ps1; what is here is what each of them
# shares: running an operation, its record as objects, a refusal as an error record, and
# completion through `lum`.
#
# Windows PowerShell 5.1 and PowerShell 7 alike: no classes, no `??`.

. (Join-Path $PSScriptRoot 'Lumenna.Rpc.ps1')
. (Join-Path $PSScriptRoot 'Lumenna.Operations.ps1')

# The servers stop with the module.
$ExecutionContext.SessionState.Module.OnRemove = { Stop-LumennaServers }

# The listings a record holds, by its `result`: each item is written to the pipeline by itself,
# so `Get-LumennaTask | Complete-LumennaTask` binds each task's id.
$script:Listings = @{
    rows       = @{ Property = 'rows'; Type = 'Lumenna.Row' }
    filters    = @{ Property = 'filters'; Type = 'Lumenna.Filter' }
    choices    = @{ Property = 'choices'; Type = 'Lumenna.Choice' }
    operations = @{ Property = 'operations'; Type = 'Lumenna.Operation' }
}

# The profile a cmdlet was given, as a whole path: relative to PowerShell's location, which is
# not the process's.
function Resolve-LumennaProfile($Cmdlet, [string] $ProfileDirectory) {
    if (-not $ProfileDirectory) { return $null }
    $Cmdlet.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ProfileDirectory)
}

function New-LumennaError([string] $Reason, [string] $Id, [System.Management.Automation.ErrorCategory] $Category, $Target) {
    $exception = New-Object System.InvalidOperationException $Reason
    $record = New-Object System.Management.Automation.ErrorRecord $exception, $Id, $Category, $Target
    $record.ErrorDetails = New-Object System.Management.Automation.ErrorDetails $Reason
    $record
}

# Asks the profile's server, and gives back its record, or writes its refusal as an error record
# carrying the core's sentence and gives back nothing.
function Invoke-LumennaRequest {
    param(
        $Cmdlet,
        [string] $Method,
        $Params = @{},
        [string] $ProfileDirectory,
        [string] $ErrorId,
        $Target
    )
    $reply = Invoke-LumennaRpc -Method $Method -Params $Params -ProfileDirectory $ProfileDirectory
    if ($null -eq $reply) {
        $Cmdlet.ThrowTerminatingError((New-LumennaError 'lum was not found: install Lumenna, or put lum on PATH.' 'Lumenna.LumNotFound' ObjectNotFound $null))
    }
    if ($reply.PSObject.Properties['error']) {
        $Cmdlet.WriteError((New-LumennaError $reply.error.message $ErrorId InvalidOperation $Target))
        return
    }
    $reply.result
}

# A record from the envelope as objects: a listing's items one by one, anything else whole, each
# named `Lumenna.<Kind>` for its format. Notices are warnings; a listing's announcement, and the
# filter's readback, are verbose.
function Write-LumennaRecord($Cmdlet, $Response) {
    $notices = $Response.PSObject.Properties['notices']
    if ($notices) { foreach ($notice in @($notices.Value)) { $Cmdlet.WriteWarning($notice) } }
    $kind = $Response.result
    $listing = $script:Listings[$kind]
    if ($listing) {
        $query = $Response.PSObject.Properties['query']
        if ($query -and $query.Value) { $Cmdlet.WriteVerbose($query.Value.description) }
        $Cmdlet.WriteVerbose($Response.announcement)
        foreach ($item in @($Response.($listing.Property))) {
            $item.PSObject.TypeNames.Insert(0, $listing.Type)
            $Cmdlet.WriteObject($item)
        }
        return
    }
    foreach ($envelope in 'version', 'result', 'notices') { $Response.PSObject.Properties.Remove($envelope) }
    $type = (($kind -split '_') | ForEach-Object { $_.Substring(0, 1).ToUpperInvariant() + $_.Substring(1) }) -join ''
    $Response.PSObject.TypeNames.Insert(0, "Lumenna.$type")
    $Cmdlet.WriteObject($Response)
}

# What a generated cmdlet does: its parameters as the operation's, by the table's names; -WhatIf
# and -Confirm on whatever writes, `confirm` given once -Confirm is answered; then the operation.
function Invoke-LumennaAssist($Cmdlet, [string] $Operation, $Bound) {
    $op = $script:Operations[$Operation]
    $given = @{}
    foreach ($name in $op.Names.Keys) {
        if (-not $Bound.ContainsKey($name)) { continue }
        $value = $Bound[$name]
        if ($value -is [System.Management.Automation.SwitchParameter]) {
            $value = $value.IsPresent
        } elseif ($value -is [array]) {
            # A plain array: one wrapped as a PSObject is written by Windows PowerShell's
            # ConvertTo-Json as an object with `value` and `Count`.
            $value = [object[]] @($value | ForEach-Object { $_ })
        }
        $given[$op.Names[$name]] = $value
    }
    $target = if ($op.Target -and $Bound.ContainsKey($op.Target)) { [string] $Bound[$op.Target] } else { $null }
    if ($op.Writes) {
        $asked = if ($target) { $Cmdlet.ShouldProcess($target, $op.Title) } else { $Cmdlet.ShouldProcess($op.Title) }
        if (-not $asked) { return }
    }
    if ($op.Confirms) { $given['confirm'] = $true }
    $record = Invoke-LumennaRequest -Cmdlet $Cmdlet -Method assist -Params @{ operation = $Operation; args = $given } `
        -ProfileDirectory (Resolve-LumennaProfile $Cmdlet $Bound['Profile']) -ErrorId "Lumenna.$Operation" -Target $target
    if ($record) { Write-LumennaRecord $Cmdlet $record }
}

# Completes a quick-add line, a filter, a project's or a label's name through the server's
# `complete`, as one types: the whole argument offered again with the completion in place,
# quoted for PowerShell. Nothing at all when `lum` cannot say.
function Complete-LumennaArgument {
    param([string] $Kind, $CommandName, $ParameterName, [string] $WordToComplete, $CommandAst, $FakeBoundParameters)
    try {
        $typed = $WordToComplete
        if ($typed -match "^['`"]") { $typed = $typed.Substring(1) }
        if ($typed.Length -and $typed -match "['`"]$") { $typed = $typed.Substring(0, $typed.Length - 1) }
        $typed = $typed -replace "''", "'"
        $profileDirectory = $null
        if ($FakeBoundParameters -and $FakeBoundParameters.Contains('Profile')) {
            $profileDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($FakeBoundParameters['Profile'])
        }
        $reply = Invoke-LumennaRpc -Method complete -Params @{ kind = $Kind; text = $typed } -ProfileDirectory $profileDirectory
        if (-not $reply -or $reply.PSObject.Properties['error']) { return }
        $found = $reply.result
        # The span is in UTF-8 bytes.
        $utf8 = New-Object System.Text.UTF8Encoding $false
        $bytes = $utf8.GetBytes($typed)
        $start = [Math]::Min([int] $found.start, $bytes.Length)
        $end = [Math]::Min([int] $found.end, $bytes.Length)
        $before = $utf8.GetString($bytes, 0, $start)
        $after = $utf8.GetString($bytes, $end, $bytes.Length - $end)
        foreach ($candidate in @($found.candidates)) {
            $whole = $before + $candidate.text + $after
            $text = if ($whole -match '^[\w.-]+$') { $whole } else { "'" + $whole.Replace("'", "''") + "'" }
            New-Object System.Management.Automation.CompletionResult $text, $candidate.text, 'ParameterValue', $candidate.label
        }
    } catch {
        # Completion that fails offers nothing, rather than an error at every keystroke.
    }
}

function Get-LumennaOperation {
    <#
    .SYNOPSIS
    Every operation an assistant can do, with the cmdlet that runs it here.

    .DESCRIPTION
    The core's assistant table, as `lum assist --list` writes it: each operation's id, its name,
    what it does, its category, whether it writes, removes something or asks first, and what it
    takes; with the cmdlet in this module that runs it. Siri, Gemini and `lum mcp` offer the same
    operations.

    .PARAMETER Name
    Only the operations whose id or cmdlet matches this, wildcards allowed.

    .LINK
    Invoke-LumennaOperation
    #>
    [CmdletBinding()]
    param(
        [Parameter(Position = 0)]
        [SupportsWildcards()]
        [string] $Name = '*'
    )
    $table = Invoke-LumennaRequest -Cmdlet $PSCmdlet -Method 'assist.list' -ErrorId 'Lumenna.List'
    foreach ($op in @($table.operations)) {
        $cmdlet = $null
        if ($script:Operations.Contains($op.id)) {
            $known = $script:Operations[$op.id]
            $cmdlet = if ($known.Switch) { "$($known.Cmdlet) -$($known.Switch)" } else { $known.Cmdlet }
        }
        if ($op.id -notlike $Name -and $cmdlet -notlike $Name) { continue }
        $op | Add-Member -NotePropertyName cmdlet -NotePropertyValue $cmdlet
        $op.PSObject.TypeNames.Insert(0, 'Lumenna.Operation')
        $op
    }
}

function Invoke-LumennaOperation {
    <#
    .SYNOPSIS
    Runs an operation of the assistant table by its id, with its parameters by the table's names.

    .DESCRIPTION
    What each of this module's cmdlets does, for an operation by its id: the parameters are the
    table's own names (`Get-LumennaOperation` lists them), and the record comes back as the
    cmdlets' do. Nothing asks first: an operation that erases permanently runs only with
    confirm = $true among its arguments.

    .PARAMETER Operation
    The operation's id, such as list_tasks.

    .PARAMETER Arguments
    Its parameters, by the table's names: @{ query = '@calls' }.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .EXAMPLE
    Invoke-LumennaOperation list_tasks @{ query = 'overdue' }

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0)]
        [string] $Operation,

        [Parameter(Position = 1)]
        [hashtable] $Arguments = @{},

        [Parameter()]
        [string] $Profile
    )
    $record = Invoke-LumennaRequest -Cmdlet $PSCmdlet -Method assist -Params @{ operation = $Operation; args = $Arguments } `
        -ProfileDirectory (Resolve-LumennaProfile $PSCmdlet $Profile) -ErrorId "Lumenna.$Operation" -Target $Operation
    if ($record) { Write-LumennaRecord $PSCmdlet $record }
}

# The completers, registered here so that each runs in the module and can reach its functions,
# which a scriptblock in a parameter's attribute cannot.
$completer = {
    param($commandName, $parameterName, $wordToComplete, $commandAst, $fakeBoundParameters)
    $kind = $script:Completed["$commandName/$parameterName"]
    if ($kind) { Complete-LumennaArgument $kind $commandName $parameterName $wordToComplete $commandAst $fakeBoundParameters }
}
foreach ($key in $script:Completed.Keys) {
    $command, $parameter = $key -split '/'
    Register-ArgumentCompleter -CommandName $command -ParameterName $parameter -ScriptBlock $completer
}
Register-ArgumentCompleter -CommandName Invoke-LumennaOperation -ParameterName Operation -ScriptBlock {
    param($commandName, $parameterName, $wordToComplete)
    foreach ($id in $script:Operations.Keys) {
        if ($id -like "$wordToComplete*") {
            New-Object System.Management.Automation.CompletionResult $id, $id, 'ParameterValue', $script:Operations[$id].Title
        }
    }
}

Export-ModuleMember -Function (@($script:Operations.Values | ForEach-Object { $_.Cmdlet } | Select-Object -Unique) + 'Get-LumennaOperation', 'Invoke-LumennaOperation')
