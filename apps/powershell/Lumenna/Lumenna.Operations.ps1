# Written by Write-Operations.ps1 from the assistant table (`lum assist --list`) and the
# module's naming of it (Cmdlets.psd1). Do not edit this: run that script again, which the
# Pester tests check was done.

# Each operation, by its id: its cmdlet, what it does to the store, and its parameters' names
# here against the table's.
$script:Operations = [ordered]@{
    'list_tasks' = @{ Cmdlet = 'Get-LumennaTask'; Switch = $null; Title = 'List Tasks'; Writes = $false; Confirms = $false; Target = $null; Names = @{ 'Filter' = 'query' } }
    'search_tasks' = @{ Cmdlet = 'Search-LumennaTask'; Switch = $null; Title = 'Search Tasks'; Writes = $false; Confirms = $false; Target = 'Text'; Names = @{ 'Text' = 'text' } }
    'show_task' = @{ Cmdlet = 'Get-LumennaTaskDetail'; Switch = $null; Title = 'Show a Task'; Writes = $false; Confirms = $false; Target = 'Id'; Names = @{ 'Id' = 'id' } }
    'day' = @{ Cmdlet = 'Get-LumennaDay'; Switch = $null; Title = 'Show the Day'; Writes = $false; Confirms = $false; Target = $null; Names = @{ 'Date' = 'date' } }
    'list_blocks' = @{ Cmdlet = 'Get-LumennaBlock'; Switch = $null; Title = 'List Blocks'; Writes = $false; Confirms = $false; Target = $null; Names = @{} }
    'list_projects' = @{ Cmdlet = 'Get-LumennaProject'; Switch = $null; Title = 'List Projects'; Writes = $false; Confirms = $false; Target = $null; Names = @{} }
    'list_labels' = @{ Cmdlet = 'Get-LumennaLabel'; Switch = $null; Title = 'List Labels'; Writes = $false; Confirms = $false; Target = $null; Names = @{} }
    'list_filters' = @{ Cmdlet = 'Get-LumennaFilter'; Switch = $null; Title = 'List Saved Filters'; Writes = $false; Confirms = $false; Target = $null; Names = @{} }
    'what_should_i_work_on' = @{ Cmdlet = 'Get-LumennaChoice'; Switch = $null; Title = 'What Should I Work On?'; Writes = $false; Confirms = $false; Target = $null; Names = @{} }
    'propose_replan' = @{ Cmdlet = 'Get-LumennaReplan'; Switch = $null; Title = 'Propose a Replan of Today'; Writes = $false; Confirms = $false; Target = $null; Names = @{} }
    'propose_day_plan' = @{ Cmdlet = 'Get-LumennaDayPlan'; Switch = $null; Title = 'Propose a Plan for a Day'; Writes = $false; Confirms = $false; Target = $null; Names = @{ 'Date' = 'date' } }
    'add_task' = @{ Cmdlet = 'Add-LumennaTask'; Switch = $null; Title = 'Add a Task'; Writes = $true; Confirms = $false; Target = 'Text'; Names = @{ 'Text' = 'text' } }
    'edit_task' = @{ Cmdlet = 'Set-LumennaTask'; Switch = $null; Title = 'Edit a Task'; Writes = $true; Confirms = $false; Target = 'Id'; Names = @{ 'Id' = 'id'; 'Title' = 'title'; 'Due' = 'due'; 'Repeat' = 'repeat'; 'Priority' = 'priority'; 'Estimate' = 'estimate'; 'Notes' = 'notes'; 'Project' = 'project'; 'Labels' = 'labels' } }
    'complete_task' = @{ Cmdlet = 'Complete-LumennaTask'; Switch = $null; Title = 'Complete a Task'; Writes = $true; Confirms = $false; Target = 'Id'; Names = @{ 'Id' = 'id' } }
    'uncomplete_task' = @{ Cmdlet = 'Undo-LumennaTaskCompletion'; Switch = $null; Title = 'Mark a Task Not Done'; Writes = $true; Confirms = $false; Target = 'Id'; Names = @{ 'Id' = 'id' } }
    'trash_task' = @{ Cmdlet = 'Remove-LumennaTask'; Switch = $null; Title = 'Move a Task to the Trash'; Writes = $true; Confirms = $false; Target = 'Id'; Names = @{ 'Id' = 'id' } }
    'erase_task_permanently' = @{ Cmdlet = 'Remove-LumennaTask'; Switch = 'Permanently'; Title = 'Erase a Task Permanently'; Writes = $true; Confirms = $true; Target = 'Id'; Names = @{ 'Id' = 'id' } }
    'erase_everything_deleted' = @{ Cmdlet = 'Clear-LumennaTrash'; Switch = $null; Title = 'Erase Everything Deleted'; Writes = $true; Confirms = $true; Target = $null; Names = @{} }
    'restore_task' = @{ Cmdlet = 'Restore-LumennaTask'; Switch = $null; Title = 'Restore a Task'; Writes = $true; Confirms = $false; Target = 'Id'; Names = @{ 'Id' = 'id' } }
    'move_task' = @{ Cmdlet = 'Move-LumennaTask'; Switch = $null; Title = 'Move a Task'; Writes = $true; Confirms = $false; Target = 'Id'; Names = @{ 'Id' = 'id'; 'Parent' = 'parent'; 'Project' = 'project'; 'Top' = 'top' } }
    'assign' = @{ Cmdlet = 'Add-LumennaSitting'; Switch = $null; Title = 'Put a Task in a Block'; Writes = $true; Confirms = $false; Target = 'Task'; Names = @{ 'Task' = 'task'; 'Block' = 'block'; 'Date' = 'date'; 'Minutes' = 'minutes' } }
    'unassign' = @{ Cmdlet = 'Remove-LumennaSitting'; Switch = $null; Title = 'Take a Task out of a Block'; Writes = $true; Confirms = $false; Target = 'Sitting'; Names = @{ 'Sitting' = 'assignment' } }
    'start_timer' = @{ Cmdlet = 'Start-LumennaTimer'; Switch = $null; Title = 'Start a Timer'; Writes = $true; Confirms = $false; Target = 'Sitting'; Names = @{ 'Sitting' = 'assignment' } }
    'pause_timer' = @{ Cmdlet = 'Suspend-LumennaTimer'; Switch = $null; Title = 'Pause a Timer'; Writes = $true; Confirms = $false; Target = 'Sitting'; Names = @{ 'Sitting' = 'assignment' } }
    'stop_timer' = @{ Cmdlet = 'Stop-LumennaTimer'; Switch = $null; Title = 'Stop a Timer'; Writes = $true; Confirms = $false; Target = 'Sitting'; Names = @{ 'Sitting' = 'assignment'; 'Minutes' = 'minutes' } }
    'start_on' = @{ Cmdlet = 'Start-LumennaChoice'; Switch = $null; Title = 'Start on a Choice'; Writes = $true; Confirms = $false; Target = 'Choice'; Names = @{ 'Choice' = 'choice' } }
    'apply_replan' = @{ Cmdlet = 'Approve-LumennaReplan'; Switch = $null; Title = 'Apply a Replan'; Writes = $true; Confirms = $false; Target = 'Proposal'; Names = @{ 'Proposal' = 'proposal'; 'Accepted' = 'accepted' } }
    'apply_day_plan' = @{ Cmdlet = 'Approve-LumennaDayPlan'; Switch = $null; Title = 'Apply a Day Plan'; Writes = $true; Confirms = $false; Target = 'Proposal'; Names = @{ 'Proposal' = 'proposal'; 'Accepted' = 'accepted' } }
    'undo' = @{ Cmdlet = 'Undo-LumennaChange'; Switch = $null; Title = 'Undo'; Writes = $true; Confirms = $false; Target = $null; Names = @{} }
    'redo' = @{ Cmdlet = 'Redo-LumennaChange'; Switch = $null; Title = 'Redo'; Writes = $true; Confirms = $false; Target = $null; Names = @{} }
}

# What `lum complete` completes, by cmdlet and parameter: registered by the module, whose own
# functions a completer in a parameter's attribute could not call.
$script:Completed = [ordered]@{
    'Get-LumennaTask/Filter' = 'filter'
    'Add-LumennaTask/Text' = 'line'
    'Set-LumennaTask/Project' = 'project'
    'Set-LumennaTask/Labels' = 'label'
    'Move-LumennaTask/Project' = 'project'
}

function Get-LumennaTask {
    <#
    .SYNOPSIS
    List Tasks

    .DESCRIPTION
    Lists tasks: by default every open task, most urgent first, subtasks under their task; or those a filter admits.

    .PARAMETER Filter
    A filter in Lumenna's filter language, such as `overdue`, `today & #Work`, `@calls`, `p1`, or `search: words`; empty for every open task.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Get-LumennaTask is the assistant operation list_tasks (lum assist list_tasks), also found by: tasks, to do, overdue, due.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding()]
    param(
        [Parameter(Position = 0)]
        [Alias('Query')]
        [string]
        $Filter,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'list_tasks' $PSBoundParameters
    }
}

function Search-LumennaTask {
    <#
    .SYNOPSIS
    Search Tasks

    .DESCRIPTION
    Searches task titles and notes for words.

    .PARAMETER Text
    Words to search titles and notes for.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Search-LumennaTask is the assistant operation search_tasks (lum assist search_tasks), also found by: find, search.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0)]
        [string]
        $Text,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'search_tasks' $PSBoundParameters
    }
}

function Get-LumennaTaskDetail {
    <#
    .SYNOPSIS
    Show a Task

    .DESCRIPTION
    One task whole: its fields, states, subtasks, what it waits for, its sittings and its history.

    .PARAMETER Id
    The task's identifier: a whole UUID, or a prefix that names one task.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Get-LumennaTaskDetail is the assistant operation show_task (lum assist show_task), also found by: details, notes.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipelineByPropertyName = $true)]
        [string]
        $Id,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'show_task' $PSBoundParameters
    }
}

function Get-LumennaDay {
    <#
    .SYNOPSIS
    Show the Day

    .DESCRIPTION
    A day as it is lived: its blocks in time order, each with the tasks in it (its sittings), the free time between them, and where now falls.

    .PARAMETER Date
    The day, as a person says it ("tomorrow", "friday", "2026-11-03"); today if left out.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Get-LumennaDay is the assistant operation day (lum assist day), also found by: today, schedule, agenda, blocks, plan.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding()]
    param(
        [Parameter(Position = 0)]
        [string]
        $Date,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'day' $PSBoundParameters
    }
}

function Get-LumennaBlock {
    <#
    .SYNOPSIS
    List Blocks

    .DESCRIPTION
    Every time block, each with when it happens and how often.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Get-LumennaBlock is the assistant operation list_blocks (lum assist list_blocks), also found by: blocks, routine, schedule.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding()]
    param(
        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'list_blocks' $PSBoundParameters
    }
}

function Get-LumennaProject {
    <#
    .SYNOPSIS
    List Projects

    .DESCRIPTION
    Every project, its subprojects under it.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Get-LumennaProject is the assistant operation list_projects (lum assist list_projects), also found by: projects.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding()]
    param(
        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'list_projects' $PSBoundParameters
    }
}

function Get-LumennaLabel {
    <#
    .SYNOPSIS
    List Labels

    .DESCRIPTION
    Every label.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Get-LumennaLabel is the assistant operation list_labels (lum assist list_labels), also found by: labels, tags.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding()]
    param(
        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'list_labels' $PSBoundParameters
    }
}

function Get-LumennaFilter {
    <#
    .SYNOPSIS
    List Saved Filters

    .DESCRIPTION
    Every saved filter, with its query.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Get-LumennaFilter is the assistant operation list_filters (lum assist list_filters), also found by: filters, views.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding()]
    param(
        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'list_filters' $PSBoundParameters
    }
}

function Get-LumennaChoice {
    <#
    .SYNOPSIS
    What Should I Work On?

    .DESCRIPTION
    What to work on now, ranked, each choice with the reason it is offered: in a block under way its own unfinished sittings, then its suggestions; in free time, what fits before the next block. Starting on one is Start-LumennaChoice.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Get-LumennaChoice is the assistant operation what_should_i_work_on (lum assist what_should_i_work_on), also found by: next, focus, now, suggest.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding()]
    param(
        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'what_should_i_work_on' $PSBoundParameters
    }
}

function Get-LumennaReplan {
    <#
    .SYNOPSIS
    Propose a Replan of Today

    .DESCRIPTION
    Proposes how to replan the rest of today, when the day has slipped: every change as a whole sentence. Nothing changes until Approve-LumennaReplan is given the ones the person accepted; say them to the person first.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Get-LumennaReplan is the assistant operation propose_replan (lum assist propose_replan), also found by: replan, behind, reschedule, running late.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding()]
    param(
        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'propose_replan' $PSBoundParameters
    }
}

function Get-LumennaDayPlan {
    <#
    .SYNOPSIS
    Propose a Plan for a Day

    .DESCRIPTION
    Proposes which tasks to put in each of a day's work blocks still to come, each with why. Nothing changes until Approve-LumennaDayPlan is given the sittings the person accepted.

    .PARAMETER Date
    The day, as a person says it ("tomorrow", "friday", "2026-11-03"); today if left out.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Get-LumennaDayPlan is the assistant operation propose_day_plan (lum assist propose_day_plan), also found by: plan the day, fill, schedule.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding()]
    param(
        [Parameter(Position = 0)]
        [string]
        $Date,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'propose_day_plan' $PSBoundParameters
    }
}

function Add-LumennaTask {
    <#
    .SYNOPSIS
    Add a Task

    .DESCRIPTION
    Adds a task (or, from a line starting with `block`, a time block) from one quick-add line, and says how it read it.

    .PARAMETER Text
    A quick-add line, read as a person's would be: the title, then any of a due date or time ("tomorrow at 3pm", "every weekday"), a priority (p1 to p4), labels (@calls), a project (#Work, #"Home Office"), an estimate (~30m). A line starting with `block` makes a time block instead ("block Deep work 9-11am weekdays"). The answer says how it was read.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Add-LumennaTask is the assistant operation add_task (lum assist add_task), also found by: new, create, remember, capture, remind.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter(Mandatory = $true, Position = 0)]
        [string]
        $Text,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'add_task' $PSBoundParameters
    }
}

function Set-LumennaTask {
    <#
    .SYNOPSIS
    Edit a Task

    .DESCRIPTION
    Changes a task's fields: only those given.

    .PARAMETER Id
    The task's identifier: a whole UUID, or a prefix that names one task.

    .PARAMETER Title
    A new title.

    .PARAMETER Due
    A date phrase ("next monday", "in 3 days at 9am"), or `none` to clear it.

    .PARAMETER Repeat
    A repetition ("every monday", "every! 2 weeks"), or `none` to stop it repeating.

    .PARAMETER Priority
    1 to 4, where 1 is highest.

    .PARAMETER Estimate
    How long it should take ("45m", "1h30m"), or `none`.

    .PARAMETER Notes
    Its notes, whole.

    .PARAMETER Project
    The project to move it to, by name.

    .PARAMETER Labels
    Its labels, all of them, by name.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Set-LumennaTask is the assistant operation edit_task (lum assist edit_task), also found by: change, rename, reschedule, due date.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipelineByPropertyName = $true)]
        [string]
        $Id,

        [Parameter()]
        [string]
        $Title,

        [Parameter()]
        [string]
        $Due,

        [Parameter()]
        [string]
        $Repeat,

        [Parameter()]
        [ValidateRange(1, 4)] [int]
        $Priority,

        [Parameter()]
        [string]
        $Estimate,

        [Parameter()]
        [string]
        $Notes,

        [Parameter()]
        [string]
        $Project,

        [Parameter()]
        [string[]]
        $Labels,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'edit_task' $PSBoundParameters
    }
}

function Complete-LumennaTask {
    <#
    .SYNOPSIS
    Complete a Task

    .DESCRIPTION
    Marks a task done. A repeating one moves on to its next date.

    .PARAMETER Id
    The task's identifier: a whole UUID, or a prefix that names one task.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Complete-LumennaTask is the assistant operation complete_task (lum assist complete_task), also found by: done, finish, check off, tick.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipelineByPropertyName = $true)]
        [string]
        $Id,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'complete_task' $PSBoundParameters
    }
}

function Undo-LumennaTaskCompletion {
    <#
    .SYNOPSIS
    Mark a Task Not Done

    .DESCRIPTION
    Marks a done task not done.

    .PARAMETER Id
    The task's identifier: a whole UUID, or a prefix that names one task.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Undo-LumennaTaskCompletion is the assistant operation uncomplete_task (lum assist uncomplete_task), also found by: undone, reopen.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipelineByPropertyName = $true)]
        [string]
        $Id,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'uncomplete_task' $PSBoundParameters
    }
}

function Remove-LumennaTask {
    <#
    .SYNOPSIS
    Move a Task to the Trash; with -Permanently, Erase a task permanently

    .DESCRIPTION
    Moves a task to the trash, from which Restore-LumennaTask brings it back.

    With -Permanently: Erases a task in the trash permanently: from the history every device keeps, and with it the history of everything deleted before. Nothing can undo it, so without `confirm` it answers with the question to ask the person first.

    .PARAMETER Id
    The task's identifier: a whole UUID, or a prefix that names one task.

    .PARAMETER Permanently
    Runs Erase a Task Permanently instead of Move a Task to the Trash.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Remove-LumennaTask is the assistant operation trash_task (lum assist trash_task), also found by: delete, remove, trash.
    Remove-LumennaTask -Permanently is the assistant operation erase_task_permanently (lum assist erase_task_permanently), also found by: erase, permanently, forever.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(DefaultParameterSetName = 'trash_task', SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
    param(
        [Parameter(ParameterSetName = 'trash_task', Mandatory = $true, Position = 0, ValueFromPipelineByPropertyName = $true)]
        [Parameter(ParameterSetName = 'erase_task_permanently', Mandatory = $true, Position = 0, ValueFromPipelineByPropertyName = $true)]
        [string]
        $Id,

        [Parameter(ParameterSetName = 'erase_task_permanently', Mandatory = $true)]
        [switch]
        $Permanently,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        $operation = $PSCmdlet.ParameterSetName
        # Erase a Task Permanently asks first as ConfirmImpact 'High' would.
        if ($operation -eq 'erase_task_permanently' -and $ConfirmPreference -ne 'None' -and $ConfirmPreference -le 'High') { $ConfirmPreference = 'Medium' }
        Invoke-LumennaAssist $PSCmdlet $operation $PSBoundParameters
    }
}

function Clear-LumennaTrash {
    <#
    .SYNOPSIS
    Erase Everything Deleted

    .DESCRIPTION
    Erases everything deleted permanently: every task in the trash, and the history of everything ever deleted, on every device. Nothing can undo it, so without `confirm` it answers with the question to ask the person first.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Clear-LumennaTrash is the assistant operation erase_everything_deleted (lum assist erase_everything_deleted), also found by: erase, empty the trash, permanently.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'High')]
    param(
        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'erase_everything_deleted' $PSBoundParameters
    }
}

function Restore-LumennaTask {
    <#
    .SYNOPSIS
    Restore a Task

    .DESCRIPTION
    Brings a task back from the trash.

    .PARAMETER Id
    The task's identifier: a whole UUID, or a prefix that names one task.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Restore-LumennaTask is the assistant operation restore_task (lum assist restore_task), also found by: undelete, bring back, restore.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipelineByPropertyName = $true)]
        [string]
        $Id,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'restore_task' $PSBoundParameters
    }
}

function Move-LumennaTask {
    <#
    .SYNOPSIS
    Move a Task

    .DESCRIPTION
    Makes a task a subtask of another, moves it to a project, or makes it a task of its own.

    .PARAMETER Id
    The task's identifier: a whole UUID, or a prefix that names one task.

    .PARAMETER Parent
    A task to make it a subtask of, by identifier.

    .PARAMETER Project
    A project to move it to, by name.

    .PARAMETER Top
    True to make it a task of its own again, under no other.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Move-LumennaTask is the assistant operation move_task (lum assist move_task), also found by: subtask, project, move.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipelineByPropertyName = $true)]
        [string]
        $Id,

        [Parameter()]
        [string]
        $Parent,

        [Parameter()]
        [string]
        $Project,

        [Parameter()]
        [switch]
        $Top,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'move_task' $PSBoundParameters
    }
}

function Add-LumennaSitting {
    <#
    .SYNOPSIS
    Put a Task in a Block

    .DESCRIPTION
    Puts a task in a block, as a sitting: one session of work on it there.

    .PARAMETER Task
    The task's identifier.

    .PARAMETER Block
    The block's identifier, as the day lists it.

    .PARAMETER Date
    The day, for a block that repeats; the block's own day otherwise.

    .PARAMETER Minutes
    How long the sitting is planned to take, in minutes.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Add-LumennaSitting is the assistant operation assign (lum assist assign), also found by: schedule, block, time block, plan.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipelineByPropertyName = $true)]
        [Alias('Id')]
        [string]
        $Task,

        [Parameter(Mandatory = $true, Position = 1)]
        [string]
        $Block,

        [Parameter()]
        [string]
        $Date,

        [Parameter()]
        [ValidateRange(0, 2147483647)] [int]
        $Minutes,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'assign' $PSBoundParameters
    }
}

function Remove-LumennaSitting {
    <#
    .SYNOPSIS
    Take a Task out of a Block

    .DESCRIPTION
    Takes a sitting out of its block; the task itself stays.

    .PARAMETER Sitting
    The sitting's identifier: a task's place in a block, as the day lists it.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Remove-LumennaSitting is the assistant operation unassign (lum assist unassign), also found by: unschedule, remove from block.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipelineByPropertyName = $true)]
        [Alias('Id', 'Assignment')]
        [string]
        $Sitting,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'unassign' $PSBoundParameters
    }
}

function Start-LumennaTimer {
    <#
    .SYNOPSIS
    Start a Timer

    .DESCRIPTION
    Starts, or resumes, a sitting's timer.

    .PARAMETER Sitting
    The sitting's identifier: a task's place in a block, as the day lists it.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Start-LumennaTimer is the assistant operation start_timer (lum assist start_timer), also found by: start, begin, work on, resume.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipelineByPropertyName = $true)]
        [Alias('Id', 'Assignment')]
        [string]
        $Sitting,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'start_timer' $PSBoundParameters
    }
}

function Suspend-LumennaTimer {
    <#
    .SYNOPSIS
    Pause a Timer

    .DESCRIPTION
    Pauses a sitting's timer, keeping what it has logged.

    .PARAMETER Sitting
    The sitting's identifier: a task's place in a block, as the day lists it.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Suspend-LumennaTimer is the assistant operation pause_timer (lum assist pause_timer), also found by: pause, break.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipelineByPropertyName = $true)]
        [Alias('Id', 'Assignment')]
        [string]
        $Sitting,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'pause_timer' $PSBoundParameters
    }
}

function Stop-LumennaTimer {
    <#
    .SYNOPSIS
    Stop a Timer

    .DESCRIPTION
    Stops a sitting's timer and logs its time.

    .PARAMETER Sitting
    The sitting's identifier: a task's place in a block, as the day lists it.

    .PARAMETER Minutes
    The minutes to log instead of the timer's, when it ran on by mistake.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Stop-LumennaTimer is the assistant operation stop_timer (lum assist stop_timer), also found by: stop, finish, log time.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipelineByPropertyName = $true)]
        [Alias('Id', 'Assignment')]
        [string]
        $Sitting,

        [Parameter()]
        [ValidateRange(0, 2147483647)] [int]
        $Minutes,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'stop_timer' $PSBoundParameters
    }
}

function Start-LumennaChoice {
    <#
    .SYNOPSIS
    Start on a Choice

    .DESCRIPTION
    Starts on one of What Should I Work On?'s choices at once, its timer running: in the block under way, or in a block of its own in free time.

    .PARAMETER Choice
    The `id` of one of the choices What Should I Work On? offered.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Start-LumennaChoice is the assistant operation start_on (lum assist start_on), also found by: start, work on, focus.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipelineByPropertyName = $true)]
        [Alias('Id')]
        [string]
        $Choice,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'start_on' $PSBoundParameters
    }
}

function Approve-LumennaReplan {
    <#
    .SYNOPSIS
    Apply a Replan

    .DESCRIPTION
    Makes the replan changes the person accepted, as one change they can undo, and proposes again around what they turned down.

    .PARAMETER Proposal
    The proposal as Get-LumennaReplan returned it, whole.

    .PARAMETER Accepted
    Those of its `changes` the person accepted, each as it was proposed.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Approve-LumennaReplan is the assistant operation apply_replan (lum assist apply_replan), also found by: replan, accept.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipeline = $true)]
        [psobject]
        $Proposal,

        [Parameter(Mandatory = $true, Position = 1)]
        [psobject[]]
        $Accepted,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'apply_replan' $PSBoundParameters
    }
}

function Approve-LumennaDayPlan {
    <#
    .SYNOPSIS
    Apply a Day Plan

    .DESCRIPTION
    Puts the sittings the person accepted into their blocks, as one change they can undo, and plans again around what they turned down.

    .PARAMETER Proposal
    The proposal as Get-LumennaDayPlan returned it, whole.

    .PARAMETER Accepted
    Those of its `sittings` the person accepted, each as it was proposed.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Approve-LumennaDayPlan is the assistant operation apply_day_plan (lum assist apply_day_plan), also found by: plan, accept.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter(Mandatory = $true, Position = 0, ValueFromPipeline = $true)]
        [psobject]
        $Proposal,

        [Parameter(Mandatory = $true, Position = 1)]
        [psobject[]]
        $Accepted,

        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'apply_day_plan' $PSBoundParameters
    }
}

function Undo-LumennaChange {
    <#
    .SYNOPSIS
    Undo

    .DESCRIPTION
    Undoes the last change made on this device, whoever made it: an app, `lum`, or an assistant.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Undo-LumennaChange is the assistant operation undo (lum assist undo), also found by: undo, take back.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'undo' $PSBoundParameters
    }
}

function Redo-LumennaChange {
    <#
    .SYNOPSIS
    Redo

    .DESCRIPTION
    Redoes what was last undone.

    .PARAMETER Profile
    The profile directory to use, instead of $env:LUMENNA_PROFILE or the platform's own.

    .NOTES
    Redo-LumennaChange is the assistant operation redo (lum assist redo), also found by: redo.

    .LINK
    Get-LumennaOperation
    #>
    [CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Low')]
    param(
        [Parameter()]
        [string]
        $Profile
    )

    process {
        Invoke-LumennaAssist $PSCmdlet 'redo' $PSBoundParameters
    }
}
