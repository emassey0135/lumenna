@{
    RootModule           = 'Lumenna.psm1'
    ModuleVersion        = '0.1.0'
    GUID                 = 'ccb1b2dc-1f89-4068-9827-6633ff768dab'
    Author               = 'Elijah Massey'
    Copyright            = '(c) Elijah Massey. AGPL-3.0-or-later.'
    Description          = 'Lumenna, the task manager and day planner, in PowerShell: every operation an assistant can do as a cmdlet, over lum.'
    PowerShellVersion    = '5.1'
    CompatiblePSEditions = @('Desktop', 'Core')
    FormatsToProcess     = @('Lumenna.Format.ps1xml')

    # Every cmdlet Lumenna.Operations.ps1 defines, and the two of the module's own; the Pester
    # tests check this against both.
    FunctionsToExport    = @(
        'Get-LumennaTask'
        'Search-LumennaTask'
        'Get-LumennaTaskDetail'
        'Get-LumennaDay'
        'Get-LumennaBlock'
        'Get-LumennaProject'
        'Get-LumennaLabel'
        'Get-LumennaFilter'
        'Get-LumennaChoice'
        'Get-LumennaReplan'
        'Get-LumennaTimer'
        'Get-LumennaDayPlan'
        'Add-LumennaTask'
        'Set-LumennaTask'
        'Complete-LumennaTask'
        'Undo-LumennaTaskCompletion'
        'Remove-LumennaTask'
        'Clear-LumennaTrash'
        'Restore-LumennaTask'
        'Move-LumennaTask'
        'Add-LumennaSitting'
        'Remove-LumennaSitting'
        'Start-LumennaTimer'
        'Suspend-LumennaTimer'
        'Stop-LumennaTimer'
        'Switch-LumennaTimer'
        'Start-LumennaChoice'
        'Approve-LumennaReplan'
        'Approve-LumennaDayPlan'
        'Undo-LumennaChange'
        'Redo-LumennaChange'
        'Get-LumennaOperation'
        'Invoke-LumennaOperation'
    )
    CmdletsToExport      = @()
    VariablesToExport    = @()
    AliasesToExport      = @()

    PrivateData          = @{
        PSData = @{
            Tags       = @('Lumenna', 'Tasks', 'Planner', 'TimeBlocking', 'Todo', 'Windows', 'Linux', 'MacOS')
            LicenseUri = 'https://github.com/emassey0135/lumenna/blob/master/LICENSE'
            ProjectUri = 'https://github.com/emassey0135/lumenna'
        }
    }
}
