# Each operation of the assistant table (`lum assist --list`) as a cmdlet: the module's own
# naming, as App Intents' type names are Swift's. Everything else about a cmdlet (its help, its
# parameters and their types, -WhatIf and -Confirm) comes from the table, through
# Write-Operations.ps1.
#
#   Cmdlet      An approved verb and a noun.
#   Switch      Where one cmdlet runs two operations, the switch that picks this one.
#   Parameters  A parameter named otherwise than the table names it; the table's name stays
#               an alias.
@{
    list_tasks               = @{ Cmdlet = 'Get-LumennaTask'; Parameters = @{ query = 'Filter' } }
    search_tasks             = @{ Cmdlet = 'Search-LumennaTask' }
    show_task                = @{ Cmdlet = 'Get-LumennaTaskDetail' }
    day                      = @{ Cmdlet = 'Get-LumennaDay' }
    list_blocks              = @{ Cmdlet = 'Get-LumennaBlock' }
    list_projects            = @{ Cmdlet = 'Get-LumennaProject' }
    list_labels              = @{ Cmdlet = 'Get-LumennaLabel' }
    list_filters             = @{ Cmdlet = 'Get-LumennaFilter' }
    what_should_i_work_on    = @{ Cmdlet = 'Get-LumennaChoice' }
    propose_replan           = @{ Cmdlet = 'Get-LumennaReplan' }
    propose_day_plan         = @{ Cmdlet = 'Get-LumennaDayPlan' }
    add_task                 = @{ Cmdlet = 'Add-LumennaTask' }
    edit_task                = @{ Cmdlet = 'Set-LumennaTask' }
    complete_task            = @{ Cmdlet = 'Complete-LumennaTask' }
    uncomplete_task          = @{ Cmdlet = 'Undo-LumennaTaskCompletion' }
    trash_task               = @{ Cmdlet = 'Remove-LumennaTask' }
    erase_task_permanently   = @{ Cmdlet = 'Remove-LumennaTask'; Switch = 'Permanently' }
    erase_everything_deleted = @{ Cmdlet = 'Clear-LumennaTrash' }
    restore_task             = @{ Cmdlet = 'Restore-LumennaTask' }
    move_task                = @{ Cmdlet = 'Move-LumennaTask' }
    assign                   = @{ Cmdlet = 'Add-LumennaSitting' }
    unassign                 = @{ Cmdlet = 'Remove-LumennaSitting'; Parameters = @{ assignment = 'Sitting' } }
    start_timer              = @{ Cmdlet = 'Start-LumennaTimer'; Parameters = @{ assignment = 'Sitting' } }
    pause_timer              = @{ Cmdlet = 'Suspend-LumennaTimer'; Parameters = @{ assignment = 'Sitting' } }
    stop_timer               = @{ Cmdlet = 'Stop-LumennaTimer'; Parameters = @{ assignment = 'Sitting' } }
    start_on                 = @{ Cmdlet = 'Start-LumennaChoice' }
    apply_replan             = @{ Cmdlet = 'Approve-LumennaReplan' }
    apply_day_plan           = @{ Cmdlet = 'Approve-LumennaDayPlan' }
    undo                     = @{ Cmdlet = 'Undo-LumennaChange' }
    redo                     = @{ Cmdlet = 'Redo-LumennaChange' }
}
