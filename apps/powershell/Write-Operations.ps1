<#
.SYNOPSIS
Writes the module's cmdlets from the assistant table.

.DESCRIPTION
Reads every operation of the assistant table (`assist.list` over `lum rpc`, which is what
`lum assist --list` writes too) and the module's naming of each in Cmdlets.psd1, and writes
Lumenna/Lumenna.Operations.ps1: a cmdlet per operation, its help the table's words, its
parameters of the types the table's kinds say, -WhatIf and -Confirm on what changes the store.
Run it again whenever the table changes.

With -Check it writes nothing, and fails if the file is not what it would write: the Pester
tests run it so, so a cmdlet cannot fall behind the table.

.PARAMETER Lum
The lum to ask: the one on PATH unless given.

.PARAMETER Check
Only check that the file is what this would write.
#>
[CmdletBinding()]
param(
    [string] $Lum = 'lum',
    [switch] $Check
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$target = Join-Path $PSScriptRoot 'Lumenna/Lumenna.Operations.ps1'
$naming = Import-PowerShellDataFile (Join-Path $PSScriptRoot 'Cmdlets.psd1')

# The table, from `assist.list` as the module asks for it, through the module's own client: the
# lum given first on PATH, and a profile of its own, so no real store is opened.
. (Join-Path $PSScriptRoot 'Lumenna/Lumenna.Rpc.ps1')
$found = @(Get-Command -Name $Lum -CommandType Application -ErrorAction Ignore)
if (-not $found.Count) { throw "There is no $Lum to ask: build it (cargo build -p lumenna-cli) or pass -Lum." }
$path = $env:PATH
$scratch = Join-Path ([IO.Path]::GetTempPath()) "lumenna-operations-$([guid]::NewGuid())"
try {
    $env:PATH = (Split-Path -Parent $found[0].Path) + [IO.Path]::PathSeparator + $path
    $reply = Invoke-LumennaRpc -Method 'assist.list' -ProfileDirectory $scratch
} finally {
    Stop-LumennaServers
    $env:PATH = $path
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
if (-not $reply -or $reply.PSObject.Properties['error']) { throw "assist.list failed: $(if ($reply) { $reply.error.message })" }
$operations = @($reply.result.operations)

# Every operation has a cmdlet, every cmdlet an operation, and each verb is one PowerShell approves.
$ids = @($operations | ForEach-Object { $_.id })
foreach ($id in $ids) {
    if (-not $naming.ContainsKey($id)) { throw "The operation $id has no cmdlet: name one in Cmdlets.psd1." }
}
foreach ($id in $naming.Keys) {
    if ($ids -notcontains $id) { throw "Cmdlets.psd1 names $id, which the assistant table does not have." }
}
$approved = @(Get-Verb | ForEach-Object { $_.Verb })
foreach ($id in $naming.Keys) {
    $verb = ($naming[$id].Cmdlet -split '-')[0]
    if ($approved -notcontains $verb) { throw "$($naming[$id].Cmdlet), for $id, does not start with an approved verb." }
}

function ConvertTo-PascalCase([string] $name) {
    (($name -split '_') | ForEach-Object { $_.Substring(0, 1).ToUpperInvariant() + $_.Substring(1) }) -join ''
}

function ConvertTo-Literal([string] $text) {
    "'" + $text.Replace("'", "''") + "'"
}

# A parameter's name in PowerShell: the module's own where Cmdlets.psd1 gives one.
function Get-ParameterName($id, $parameter) {
    $renamed = $naming[$id]['Parameters']
    if ($renamed -and $renamed.ContainsKey($parameter.name)) { return $renamed[$parameter.name] }
    ConvertTo-PascalCase $parameter.name
}

# The words an assistant reads name other operations by id; a person here knows them as cmdlets.
function ConvertTo-Help([string] $text) {
    [regex]::Replace($text, '`([a-z_]+)`', {
            param($match)
            $id = $match.Groups[1].Value
            if ($naming.ContainsKey($id)) { Get-CmdletText $id } else { $match.Value }
        })
}

function Get-CmdletText($id) {
    $switch = $naming[$id]['Switch']
    if ($switch) { "$($naming[$id].Cmdlet) -$switch" } else { $naming[$id].Cmdlet }
}

$types = @{
    minutes           = '[ValidateRange(0, 2147483647)] [int]'
    priority          = '[ValidateRange(1, 4)] [int]'
    flag              = '[switch]'
    labels            = '[string[]]'
    replan            = '[psobject]'
    day_plan          = '[psobject]'
    replan_changes    = '[psobject[]]'
    day_plan_sittings = '[psobject[]]'
}
$completers = @{ line = 'line'; query = 'filter'; project = 'project'; labels = 'label' }
$identifiers = @('task', 'sitting', 'choice', 'block')
$impacts = @{ None = 0; Low = 1; Medium = 2; High = 3 }

function Get-Impact($op) {
    if ($op.confirms) { 'High' } elseif ($op.destructive) { 'Medium' } elseif ($op.writes) { 'Low' } else { 'None' }
}

# The parameters an operation takes here: all but `confirm`, which -Confirm answers.
function Get-Taken($op) {
    @($op.parameters | Where-Object { $_.name -ne 'confirm' })
}

$out = New-Object System.Text.StringBuilder
function Add-Line([string] $line = '') { [void] $out.Append($line).Append("`n") }

Add-Line '# Written by Write-Operations.ps1 from the assistant table (RPC''s `assist.list`) and the'
Add-Line '# module''s naming of it (Cmdlets.psd1). Do not edit this: run that script again, which the'
Add-Line '# Pester tests check was done.'
Add-Line
Add-Line '# Each operation, by its id: its cmdlet, what it does to the store, and its parameters'' names'
Add-Line '# here against the table''s.'
Add-Line '$script:Operations = [ordered]@{'
foreach ($op in $operations) {
    $taken = Get-Taken $op
    $names = @($taken | ForEach-Object { "$(ConvertTo-Literal (Get-ParameterName $op.id $_)) = $(ConvertTo-Literal $_.name)" }) -join '; '
    $names = if ($names) { " $names " } else { '' }
    $required = @($taken | Where-Object { -not $_.optional })
    $targetName = if ($required.Count) { ConvertTo-Literal (Get-ParameterName $op.id $required[0]) } else { '$null' }
    $switch = if ($naming[$op.id]['Switch']) { ConvertTo-Literal $naming[$op.id]['Switch'] } else { '$null' }
    Add-Line ("    $(ConvertTo-Literal $op.id) = @{ Cmdlet = $(ConvertTo-Literal $naming[$op.id].Cmdlet); Switch = $switch; " +
        "Title = $(ConvertTo-Literal $op.title); Writes = `$$($op.writes.ToString().ToLowerInvariant()); " +
        "Confirms = `$$($op.confirms.ToString().ToLowerInvariant()); Target = $targetName; Names = @{$names} }")
}
Add-Line '}'
Add-Line
Add-Line '# What `lum complete` completes, by cmdlet and parameter: registered by the module, whose own'
Add-Line '# functions a completer in a parameter''s attribute could not call.'
Add-Line '$script:Completed = [ordered]@{'
$completed = New-Object System.Collections.Specialized.OrderedDictionary
foreach ($op in $operations) {
    foreach ($parameter in (Get-Taken $op)) {
        if (-not $completers.ContainsKey($parameter.kind)) { continue }
        $key = "$($naming[$op.id].Cmdlet)/$(Get-ParameterName $op.id $parameter)"
        if (-not $completed.Contains($key)) { $completed[$key] = $completers[$parameter.kind] }
    }
}
foreach ($key in $completed.Keys) { Add-Line "    $(ConvertTo-Literal $key) = $(ConvertTo-Literal $completed[$key])" }
Add-Line '}'

# The cmdlets, in the table's order of their first operation.
$cmdlets = New-Object System.Collections.Specialized.OrderedDictionary
foreach ($op in $operations) {
    $name = $naming[$op.id].Cmdlet
    if (-not $cmdlets.Contains($name)) { $cmdlets[$name] = New-Object System.Collections.ArrayList }
    [void] $cmdlets[$name].Add($op)
}

foreach ($name in $cmdlets.Keys) {
    $group = @($cmdlets[$name])
    $base = @($group | Where-Object { -not $naming[$_.id]['Switch'] })
    if ($base.Count -ne 1) { throw "$name runs $($group.Count) operations, so all but one need a Switch in Cmdlets.psd1." }
    $base = $base[0]
    $others = @($group | Where-Object { $_ -ne $base })
    $several = $group.Count -gt 1

    # Each parameter once, with an attribute for each operation (its parameter set) that takes it.
    $parameters = New-Object System.Collections.Specialized.OrderedDictionary
    foreach ($op in $group) {
        $taken = Get-Taken $op
        $required = @($taken | Where-Object { -not $_.optional })
        $position = 0
        $first = $true
        foreach ($parameter in $taken) {
            $psName = Get-ParameterName $op.id $parameter
            if (-not $parameters.Contains($psName)) {
                $parameters[$psName] = @{ Parameter = $parameter; Sets = New-Object System.Collections.ArrayList; Aliases = New-Object System.Collections.ArrayList }
            }
            $entry = $parameters[$psName]
            $arguments = New-Object System.Collections.ArrayList
            if ($several) { [void] $arguments.Add("ParameterSetName = $(ConvertTo-Literal $op.id)") }
            if (-not $parameter.optional) { [void] $arguments.Add('Mandatory = $true') }
            # What is required comes in order without its name; with nothing required, the first.
            if ((-not $parameter.optional -or ($required.Count -eq 0 -and $first)) -and $parameter.kind -ne 'flag') {
                [void] $arguments.Add("Position = $position")
                $position++
            }
            # The record a cmdlet acts on comes down the pipeline by its id.
            if ($required.Count -and $parameter -eq $required[0] -and $identifiers -contains $parameter.kind) {
                [void] $arguments.Add('ValueFromPipelineByPropertyName = $true')
                if ($psName -ne 'Id' -and -not $entry.Aliases.Contains('Id')) { [void] $entry.Aliases.Add('Id') }
            }
            if ($parameter.kind -eq 'replan' -or $parameter.kind -eq 'day_plan') { [void] $arguments.Add('ValueFromPipeline = $true') }
            if ($psName -ne (ConvertTo-PascalCase $parameter.name) -and -not $entry.Aliases.Contains((ConvertTo-PascalCase $parameter.name))) {
                [void] $entry.Aliases.Add((ConvertTo-PascalCase $parameter.name))
            }
            [void] $entry.Sets.Add("[Parameter($($arguments -join ', '))]")
            $first = $false
        }
    }

    Add-Line
    Add-Line "function $name {"
    Add-Line '    <#'
    Add-Line '    .SYNOPSIS'
    $synopsis = $base.title
    foreach ($other in $others) { $synopsis += "; with -$($naming[$other.id].Switch), $($other.sentence)" }
    Add-Line "    $synopsis"
    Add-Line
    Add-Line '    .DESCRIPTION'
    Add-Line "    $(ConvertTo-Help $base.description)"
    foreach ($other in $others) {
        Add-Line
        Add-Line "    With -$($naming[$other.id].Switch): $(ConvertTo-Help $other.description)"
    }
    foreach ($psName in $parameters.Keys) {
        Add-Line
        Add-Line "    .PARAMETER $psName"
        Add-Line "    $(ConvertTo-Help $parameters[$psName].Parameter.description)"
    }
    foreach ($other in $others) {
        Add-Line
        Add-Line "    .PARAMETER $($naming[$other.id].Switch)"
        Add-Line "    Runs $($other.title) instead of $($base.title)."
    }
    Add-Line
    Add-Line '    .PARAMETER Profile'
    Add-Line '    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform''s own.'
    Add-Line
    Add-Line '    .NOTES'
    foreach ($op in $group) {
        Add-Line "    $(Get-CmdletText $op.id) is the assistant operation $($op.id) (lum assist $($op.id)), also found by: $($op.keywords -join ', ')."
    }
    Add-Line
    Add-Line '    .LINK'
    Add-Line '    Get-LumennaOperation'
    Add-Line '    #>'

    $binding = New-Object System.Collections.ArrayList
    if ($several) { [void] $binding.Add("DefaultParameterSetName = $(ConvertTo-Literal $base.id)") }
    $impact = Get-Impact $base
    if ($impact -ne 'None') {
        [void] $binding.Add('SupportsShouldProcess = $true')
        [void] $binding.Add("ConfirmImpact = '$impact'")
    }
    Add-Line "    [CmdletBinding($($binding -join ', '))]"
    Add-Line '    param('
    $declarations = New-Object System.Collections.ArrayList
    foreach ($psName in $parameters.Keys) {
        $entry = $parameters[$psName]
        $lines = New-Object System.Collections.ArrayList
        foreach ($set in $entry.Sets) { [void] $lines.Add("        $set") }
        if ($entry.Aliases.Count) { [void] $lines.Add("        [Alias($(@($entry.Aliases | ForEach-Object { ConvertTo-Literal $_ }) -join ', '))]") }
        $kind = $entry.Parameter.kind
        $type = if ($types.ContainsKey($kind)) { $types[$kind] } else { '[string]' }
        [void] $lines.Add("        $type")
        [void] $lines.Add("        `$$psName")
        [void] $declarations.Add($lines -join "`n")
    }
    foreach ($other in $others) {
        [void] $declarations.Add("        [Parameter(ParameterSetName = $(ConvertTo-Literal $other.id), Mandatory = `$true)]`n        [switch]`n        `$$($naming[$other.id].Switch)")
    }
    [void] $declarations.Add("        [Parameter()]`n        [string]`n        `$Profile")
    Add-Line ($declarations -join ",`n`n")
    Add-Line '    )'
    Add-Line
    Add-Line '    process {'
    if ($several) {
        Add-Line '        $operation = $PSCmdlet.ParameterSetName'
        foreach ($other in $others) {
            $raised = Get-Impact $other
            if ($impacts[$raised] -gt $impacts[$impact]) {
                # Asks as ConfirmImpact would for this operation alone: the cmdlet's own is the base's.
                Add-Line "        # $($other.title) asks first as ConfirmImpact '$raised' would."
                Add-Line "        if (`$operation -eq $(ConvertTo-Literal $other.id) -and `$ConfirmPreference -ne 'None' -and `$ConfirmPreference -le '$raised') { `$ConfirmPreference = '$impact' }"
            }
        }
        Add-Line '        Invoke-LumennaAssist $PSCmdlet $operation $PSBoundParameters'
    } else {
        Add-Line "        Invoke-LumennaAssist `$PSCmdlet $(ConvertTo-Literal $base.id) `$PSBoundParameters"
    }
    Add-Line '    }'
    Add-Line '}'
}

$written = $out.ToString()
if ($Check) {
    $current = if (Test-Path -LiteralPath $target) { [IO.File]::ReadAllText($target) -replace "`r`n", "`n" } else { '' }
    if ($current -ne $written) {
        throw "Lumenna.Operations.ps1 is not what the assistant table makes now: run apps/powershell/Write-Operations.ps1."
    }
} else {
    # With a byte order mark, which Windows PowerShell 5.1 needs to read anything but ASCII as UTF-8.
    [IO.File]::WriteAllText($target, $written, (New-Object System.Text.UTF8Encoding $true))
}
