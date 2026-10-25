# The PowerShell module against the real `lum`: that its cmdlets are the assistant table's
# operations, every one, as AssistantIntentTests and AppFunctionsTest hold App Intents and App
# Functions to it; and that they work, against a temporary profile. Pester 5, on Windows
# PowerShell 5.1 and PowerShell 7 alike (run.ps1 runs it). ASCII only: 5.1 reads a file without a
# byte order mark in the system's code page.

BeforeAll {
    $root = (Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
    # The lum this tree builds, ahead of any installed.
    $debug = Join-Path $root 'target/debug'
    if (Test-Path -LiteralPath $debug) { $env:PATH = $debug + [IO.Path]::PathSeparator + $env:PATH }
    $script:lum = @(Get-Command -Name lum -CommandType Application)[0].Path
    $script:manifest = Join-Path $root 'apps/powershell/Lumenna/Lumenna.psd1'
    $script:module = Import-Module $manifest -Force -PassThru
    $script:table = @((& $module { Invoke-Lum -Arguments '--json', 'assist', '--list' }).Output | ConvertFrom-Json | ForEach-Object { $_.operations })
    $script:mapping = & $module { $script:Operations }
    $script:helpers = @('Get-LumennaOperation', 'Invoke-LumennaOperation')

    # The table's words as the help says them: another operation by its cmdlet.
    function ConvertTo-Help([string] $text) {
        [regex]::Replace($text, '`([a-z_]+)`', {
                param($match)
                $known = $mapping[$match.Groups[1].Value]
                if (-not $known) { $match.Value } elseif ($known.Switch) { "$($known.Cmdlet) -$($known.Switch)" } else { $known.Cmdlet }
            })
    }

    function New-Profile {
        $directory = Join-Path ([IO.Path]::GetTempPath()) "lumenna-pester-$([guid]::NewGuid())"
        New-Item -ItemType Directory -Path $directory | Out-Null
        $directory
    }

    $script:shell = (Get-Process -Id $PID).Path
    # Runs a line in another process of this same PowerShell, with no one to answer a prompt.
    # Through files, not 2>&1: Windows PowerShell 5.1 makes a native command's error output an
    # error of its own here, which the prompt it refuses would be.
    function Invoke-Unattended([string] $line) {
        $script = "Import-Module '$($manifest.Replace("'", "''"))'; $line"
        $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($script))
        $said = Join-Path ([IO.Path]::GetTempPath()) "lumenna-pester-$([guid]::NewGuid())"
        try {
            Start-Process -FilePath $shell -ArgumentList '-NoProfile', '-NonInteractive', '-EncodedCommand', $encoded `
                -Wait -NoNewWindow -RedirectStandardOutput "$said.out" -RedirectStandardError "$said.err"
        } finally {
            Remove-Item -LiteralPath "$said.out", "$said.err" -ErrorAction SilentlyContinue
        }
    }

    $script:savedProfile = $env:LUMENNA_PROFILE
    $script:savedBackups = $env:LUMENNA_BACKUP_DIR
}

AfterAll {
    $env:LUMENNA_PROFILE = $savedProfile
    $env:LUMENNA_BACKUP_DIR = $savedBackups
}

Describe 'The cmdlets are the assistant table''s operations' {
    It 'are what Write-Operations.ps1 writes from the table as it is now' {
        { & (Join-Path $root 'apps/powershell/Write-Operations.ps1') -Lum $lum -Check } | Should -Not -Throw
    }

    It 'has a cmdlet for every operation lum assist --list names' {
        $table.Count | Should -BeGreaterThan 20
        $missing = @($table | Where-Object { -not $mapping.Contains($_.id) } | ForEach-Object { $_.id })
        $missing | Should -BeNullOrEmpty -Because 'each operation needs a cmdlet in Cmdlets.psd1'
        foreach ($op in $table) {
            $known = $mapping[$op.id]
            $command = Get-Command -Module Lumenna -Name $known.Cmdlet -ErrorAction SilentlyContinue
            $command | Should -Not -BeNullOrEmpty -Because "$($op.id) runs as $($known.Cmdlet)"
            if ($known.Switch) { $command.Parameters.Keys | Should -Contain $known.Switch }
        }
    }

    It 'runs an operation the table has from every cmdlet but the module''s own two' {
        $ids = @($table | ForEach-Object { $_.id })
        foreach ($id in $mapping.Keys) { $ids | Should -Contain $id -Because 'a cmdlet whose operation is gone has nothing to run' }
        $cmdlets = @($mapping.Values | ForEach-Object { $_.Cmdlet } | Sort-Object -Unique)
        $exported = @($module.ExportedFunctions.Keys | Sort-Object)
        $exported | Should -Be @(($cmdlets + $helpers) | Sort-Object)
    }

    It 'exports from the manifest exactly the cmdlets there are' {
        $listed = (Import-PowerShellDataFile $manifest).FunctionsToExport
        @($listed | Sort-Object) | Should -Be @($module.ExportedFunctions.Keys | Sort-Object)
    }

    It 'names every cmdlet with a verb PowerShell approves' {
        $approved = @(Get-Verb | ForEach-Object { $_.Verb })
        foreach ($name in $module.ExportedFunctions.Keys) { $approved | Should -Contain ($name -split '-')[0] }
    }

    It 'says the table''s words as its help' {
        foreach ($op in $table) {
            $known = $mapping[$op.id]
            $help = Get-Help $known.Cmdlet
            # Matched as text: the table's backticks and question marks are wildcards to -BeLike.
            $help.Synopsis | Should -Match ([regex]::Escape($op.title))
            (@($help.description | ForEach-Object { $_.Text }) -join "`n") | Should -Match ([regex]::Escape((ConvertTo-Help $op.description)))
            # A parameter shared with the cmdlet's first operation is said in that one's words.
            if ($known.Switch) { continue }
            foreach ($parameter in $op.parameters) {
                if ($parameter.name -eq 'confirm') { continue }
                $name = @($known.Names.Keys | Where-Object { $known.Names[$_] -eq $parameter.name })[0]
                $said = @((Get-Help $known.Cmdlet -Parameter $name).description | ForEach-Object { $_.Text }) -join ''
                $said | Should -Be (ConvertTo-Help $parameter.description) -Because "$($known.Cmdlet) -$name"
            }
        }
    }

    It 'takes each parameter as the type its kind is' {
        $types = @{
            flag = [switch]; minutes = [int]; priority = [int]; labels = [string[]]
            replan = [psobject]; day_plan = [psobject]; replan_changes = [psobject[]]; day_plan_sittings = [psobject[]]
        }
        foreach ($op in $table) {
            $known = $mapping[$op.id]
            $command = Get-Command $known.Cmdlet
            foreach ($parameter in $op.parameters) {
                if ($parameter.name -eq 'confirm') { continue }
                $name = @($known.Names.Keys | Where-Object { $known.Names[$_] -eq $parameter.name })[0]
                $expected = if ($types.ContainsKey($parameter.kind)) { $types[$parameter.kind] } else { [string] }
                $command.Parameters[$name].ParameterType | Should -Be $expected -Because "$($known.Cmdlet) -$name is $($parameter.kind)"
                $command.Parameters[$name].Attributes.Where({ $_ -is [System.Management.Automation.ParameterAttribute] }).Mandatory |
                    Should -Contain (-not $parameter.optional)
            }
        }
    }

    It 'offers -WhatIf and -Confirm on whatever writes, erasing for good at ConfirmImpact High' {
        foreach ($op in $table) {
            $known = $mapping[$op.id]
            $binding = (Get-Command $known.Cmdlet).ScriptBlock.Attributes.Where({ $_ -is [System.Management.Automation.CmdletBindingAttribute] })[0]
            $binding.SupportsShouldProcess | Should -Be ([bool] (@($mapping.Values | Where-Object { $_.Cmdlet -eq $known.Cmdlet -and $_.Writes }).Count)) -Because $known.Cmdlet
            if ($op.confirms -and -not $known.Switch) { $binding.ConfirmImpact | Should -Be 'High' -Because $known.Cmdlet }
        }
    }
}

Describe 'Round trips' {
    BeforeEach {
        $env:LUMENNA_PROFILE = New-Profile
        $env:LUMENNA_BACKUP_DIR = Join-Path $env:LUMENNA_PROFILE 'backups'
    }

    AfterEach {
        Remove-Item -LiteralPath $env:LUMENNA_PROFILE -Recurse -Force -ErrorAction SilentlyContinue
    }

    It 'adds, lists and completes down the pipeline, binding each task''s id' {
        $added = Add-LumennaTask 'call the bank @calls p1'
        $added.PSObject.TypeNames[0] | Should -Be 'Lumenna.Change'
        $added.changed | Should -BeTrue
        Add-LumennaTask 'phone the dentist @calls' | Out-Null
        Add-LumennaTask 'water the plants' | Out-Null

        $calls = @(Get-LumennaTask -Filter '@calls')
        $calls.Count | Should -Be 2
        $calls[0].PSObject.TypeNames[0] | Should -Be 'Lumenna.Row'
        $calls[0].id | Should -Be $added.task.id

        $done = @(Get-LumennaTask -Filter '@calls' | Complete-LumennaTask)
        $done.Count | Should -Be 2
        @(Get-LumennaTask | ForEach-Object { $_.title }) | Should -Be @('water the plants')
        @(Get-LumennaTask -Filter '@calls').Count | Should -Be 0
    }

    It 'keeps words outside ASCII whole both ways' {
        $title = "caf$([char] 0xE9) with Zo$([char] 0xEB)"
        (Add-LumennaTask $title).task.title | Should -Be $title
        (Get-LumennaTask)[0].title | Should -Be $title
    }

    It 'edits only what it is given, labels as a list' {
        $id = (Add-LumennaTask 'review the report').task.id
        Set-LumennaTask $id -Priority 2 -Labels 'work', 'reading' -WarningAction SilentlyContinue | Out-Null
        $task = Get-LumennaTaskDetail $id
        $task.priority | Should -Be 2
        @($task.labels | Sort-Object) | Should -Be @('reading', 'work')
        $task.title | Should -Be 'review the report'
        Set-LumennaTask $id -Labels 'work' | Out-Null
        @((Get-LumennaTaskDetail $id).labels) | Should -Be @('work')
    }

    It 'trashes and restores' {
        $id = (Add-LumennaTask 'trash me').task.id
        Get-LumennaTask | Remove-LumennaTask | Out-Null
        @(Get-LumennaTask).Count | Should -Be 0
        @(Get-LumennaTask -Filter deleted | ForEach-Object { $_.id }) | Should -Be @($id)
        Get-LumennaTask -Filter deleted | Restore-LumennaTask | Out-Null
        @(Get-LumennaTask | ForEach-Object { $_.id }) | Should -Be @($id)
    }

    It 'changes nothing with -WhatIf' {
        $id = (Add-LumennaTask 'keep me').task.id
        Remove-LumennaTask $id -WhatIf
        Set-LumennaTask $id -Title 'renamed' -WhatIf
        Complete-LumennaTask $id -WhatIf
        @(Get-LumennaTask | ForEach-Object { $_.title }) | Should -Be @('keep me')
        Remove-LumennaTask $id | Out-Null
        Remove-LumennaTask $id -Permanently -WhatIf
        Clear-LumennaTrash -WhatIf
        @(Get-LumennaTask -Filter deleted | ForEach-Object { $_.id }) | Should -Be @($id)
    }

    It 'asks before erasing permanently, and with -Confirm:$false erases' {
        $id = (Add-LumennaTask 'erase me').task.id
        # Trashing asks nothing, so it goes ahead where no one could answer.
        Invoke-Unattended "Remove-LumennaTask '$id'"
        @(Get-LumennaTask -Filter deleted | ForEach-Object { $_.id }) | Should -Be @($id)
        # Erasing asks, so where no one can answer it is not done.
        Invoke-Unattended "Remove-LumennaTask '$id' -Permanently"
        Invoke-Unattended 'Clear-LumennaTrash'
        @(Get-LumennaTask -Filter deleted | ForEach-Object { $_.id }) | Should -Be @($id)

        $erased = Remove-LumennaTask $id -Permanently -Confirm:$false
        $erased.changed | Should -BeTrue
        @(Get-LumennaTask -Filter deleted).Count | Should -Be 0
    }

    It 'says a refusal as an error record carrying the core''s sentence' {
        $errors = $null
        $result = Complete-LumennaTask -Id 'nothing-like-this' -ErrorAction SilentlyContinue -ErrorVariable errors
        $result | Should -BeNullOrEmpty
        $errors.Count | Should -Be 1
        $errors[0] | Should -BeOfType [System.Management.Automation.ErrorRecord]
        $errors[0].FullyQualifiedErrorId | Should -BeLike 'Lumenna.complete_task,*'
        $errors[0].Exception.Message | Should -BeLike '*nothing-like-this*'
        $errors[0].Exception.Message | Should -Not -BeLike 'lum:*'
        $errors[0].TargetObject | Should -Be 'nothing-like-this'
        { Complete-LumennaTask -Id 'nothing-like-this' -ErrorAction Stop } | Should -Throw
    }

    It 'uses the profile -Profile names, else LUMENNA_PROFILE' {
        $other = New-Profile
        try {
            Add-LumennaTask 'somewhere else' -Profile $other | Out-Null
            @(Get-LumennaTask -Profile $other | ForEach-Object { $_.title }) | Should -Be @('somewhere else')
            @(Get-LumennaTask).Count | Should -Be 0
        } finally {
            Remove-Item -LiteralPath $other -Recurse -Force -ErrorAction SilentlyContinue
        }
    }

    It 'puts a task in a block and takes it out again, the day''s sittings down the pipeline' {
        $task = (Add-LumennaTask 'deep work on the draft').task.id
        Add-LumennaTask 'block Writing 9am to 10am every day' | Out-Null
        $block = @((Get-LumennaDay -Date tomorrow).blocks)[0].id
        $put = Add-LumennaSitting -Task $task -Block $block -Date tomorrow -Minutes 30
        $put.changed | Should -BeTrue
        $sittings = @((Get-LumennaDay -Date tomorrow).blocks | ForEach-Object { $_.assignments })
        $sittings.Count | Should -Be 1
        $sittings[0].planned_mins | Should -Be 30
        $sittings | Remove-LumennaSitting | Out-Null
        @((Get-LumennaDay -Date tomorrow).blocks | ForEach-Object { $_.assignments }).Count | Should -Be 0
        (Undo-LumennaChange).changed | Should -BeTrue
        @((Get-LumennaDay -Date tomorrow).blocks | ForEach-Object { $_.assignments }).Count | Should -Be 1
    }

    It 'runs any operation by its id, and lists every one with its cmdlet' {
        Invoke-LumennaOperation add_task @{ text = 'by its id' } | Out-Null
        @(Invoke-LumennaOperation list_tasks | ForEach-Object { $_.title }) | Should -Be @('by its id')
        $ops = @(Get-LumennaOperation)
        $ops.Count | Should -Be $table.Count
        (Get-LumennaOperation complete_task).cmdlet | Should -Be 'Complete-LumennaTask'
        (Get-LumennaOperation erase_task_permanently).cmdlet | Should -Be 'Remove-LumennaTask -Permanently'
    }

    It 'completes the quick-add line, the filter language, projects and labels through lum' {
        Add-LumennaTask 'phone @calls' | Out-Null
        function Complete-Typed([string] $line) {
            @((TabExpansion2 -inputScript $line -cursorColumn $line.Length).CompletionMatches | ForEach-Object { $_.CompletionText })
        }
        Complete-Typed "Add-LumennaTask 'buy paper #In" | Should -Be @("'buy paper #Inbox'")
        Complete-Typed "Get-LumennaTask -Filter '@ca" | Should -Be @("'@calls'")
        Complete-Typed 'Set-LumennaTask x -Project In' | Should -Be @('Inbox')
        Complete-Typed 'Set-LumennaTask x -Labels work,ca' | Should -Be @('calls')
        Complete-Typed 'Invoke-LumennaOperation complete_t' | Should -Be @('complete_task')
    }
}
