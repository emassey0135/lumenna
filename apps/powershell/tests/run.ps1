<#
.SYNOPSIS
Runs the module's Pester tests in whichever PowerShell runs this script.

.DESCRIPTION
Windows PowerShell 5.1 ships Pester 3, so Pester 5 is taken from -Modules when it is there
(CI saves it there once, for both editions), else from wherever it is installed. The tests
use the lum in target/debug, ahead of any on PATH: build it first (cargo build -p lumenna-cli).

.PARAMETER Modules
A folder Pester 5 was saved into with Save-Module.
#>
param(
    [string] $Modules = $(if ($env:RUNNER_TEMP) { Join-Path $env:RUNNER_TEMP 'modules' } else { '' })
)

$ErrorActionPreference = 'Stop'
if ($Modules -and (Test-Path -LiteralPath $Modules)) {
    $env:PSModulePath = $Modules + [IO.Path]::PathSeparator + $env:PSModulePath
}
Import-Module Pester -MinimumVersion 5.5

"PowerShell $($PSVersionTable.PSVersion) ($($PSVersionTable.PSEdition)), Pester $((Get-Module Pester).Version)"
$configuration = New-PesterConfiguration
$configuration.Run.Path = $PSScriptRoot
$configuration.Run.Exit = $true
$configuration.Output.Verbosity = 'Detailed'
Invoke-Pester -Configuration $configuration
