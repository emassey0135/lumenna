# What Lumenna is, as the workspace's Cargo.toml says it once (`[workspace.metadata.lumenna]`, and
# the licence from `[workspace.package]`): dot-sourced by the MSIX's scripts, so a version or a name
# is changed there alone. Inno Setup's script reads the same table itself.
function Read-LumennaMetadata {
    # This file's own folder, wherever it is dot-sourced from.
    $workspace = Join-Path $PSScriptRoot '..\..\..\Cargo.toml'
    $metadata = @{}
    $section = ''
    foreach ($line in [IO.File]::ReadAllLines((Resolve-Path $workspace).Path)) {
        if ($line -match '^\s*\[(.+)\]\s*$') { $section = $Matches[1]; continue }
        if ($line -notmatch '^\s*(\w+)\s*=\s*"(.*)"\s*$') { continue }
        if ($section -eq 'workspace.metadata.lumenna') { $metadata[$Matches[1]] = $Matches[2] }
        elseif ($section -eq 'workspace.package' -and $Matches[1] -eq 'license') { $metadata['licence'] = $Matches[2] }
    }
    foreach ($key in 'name', 'description', 'version', 'publisher', 'id', 'licence') {
        if (-not $metadata[$key]) { throw "The workspace's Cargo.toml says no $key of Lumenna ([workspace.metadata.lumenna])" }
    }
    $metadata
}
