# Packs Lumenna as an MSIX: the app and `lum` from -Source, the manifest beside this script filled
# in, and its logos drawn from the app's icon at the sizes the manifest names. Unsigned: the
# Microsoft Store signs what it is given. Run by the release workflow, and by hand the same way:
#
#   powershell -File apps\windows\installer\msix\pack.ps1 -Arch x64 -Version 0.1.0 `
#     -Source target\release -Output dist
#
# Windows PowerShell, for System.Drawing.
param(
    [Parameter(Mandatory)] [ValidateSet('x64', 'arm64')] [string] $Arch,
    [Parameter(Mandatory)] [string] $Version,
    [Parameter(Mandatory)] [string] $Source,
    [Parameter(Mandatory)] [string] $Output,
    # Stand-ins until Partner Center gives the package's identity.
    [string] $IdentityName = 'dev.emassey0135.Lumenna',
    [string] $Publisher = 'CN=Elijah Massey',
    [string] $PublisherDisplayName = 'Elijah Massey'
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$here = $PSScriptRoot
$root = (Resolve-Path "$here\..\..\..\..").Path
$layout = Join-Path ([IO.Path]::GetTempPath()) "lumenna-msix-$Arch"
Remove-Item -Recurse -Force $layout -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$layout\Assets" | Out-Null
New-Item -ItemType Directory -Force $Output | Out-Null

Copy-Item "$Source\lumenna.exe", "$Source\lum.exe" $layout
Copy-Item "$root\LICENSE" "$layout\LICENSE.txt"

# A package's version has four parts; Cargo's has three.
$packageVersion = (($Version -split '[-+]')[0] + '.0')
$manifest = Get-Content -Raw "$here\AppxManifest.xml"
$manifest = $manifest.Replace('@IDENTITY_NAME@', $IdentityName).Replace('@PUBLISHER@', [Security.SecurityElement]::Escape($Publisher))
$manifest = $manifest.Replace('@PUBLISHER_DISPLAY_NAME@', [Security.SecurityElement]::Escape($PublisherDisplayName))
$manifest = $manifest.Replace('@VERSION@', $packageVersion).Replace('@ARCH@', $Arch)
[IO.File]::WriteAllText("$layout\AppxManifest.xml", $manifest, (New-Object Text.UTF8Encoding $false))

# The logos, square, from the largest of the icon's images (GTK's, which Windows' is drawn from).
$icon = [Drawing.Image]::FromFile("$root\apps\gtk\data\icons\hicolor\512x512\apps\dev.emassey0135.Lumenna.png")
# The widget's icon, and its picture in the board's picker: the icon, centred on the small
# widget's shape, 300 by 304.
$logos = @(
    @{ Name = 'StoreLogo'; Size = 50 }, @{ Name = 'Square44x44Logo'; Size = 44 }, @{ Name = 'Square150x150Logo'; Size = 150 },
    @{ Name = 'WidgetIcon'; Size = 64 }, @{ Name = 'WidgetScreenshot'; Size = 150; Width = 300; Height = 304 }
)
foreach ($logo in $logos) {
    $width = if ($logo.Width) { $logo.Width } else { $logo.Size }
    $height = if ($logo.Height) { $logo.Height } else { $logo.Size }
    $bitmap = New-Object Drawing.Bitmap $width, $height
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $graphics.DrawImage($icon, [int](($width - $logo.Size) / 2), [int](($height - $logo.Size) / 2), $logo.Size, $logo.Size)
    $graphics.Dispose()
    $bitmap.Save("$layout\Assets\$($logo.Name).png", [Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()
}
$icon.Dispose()

# MakeAppx, from the newest Windows SDK, for this machine's own architecture where it has one.
$machine = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x64' }
$sdk = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.*" -Directory | Sort-Object { [version]$_.Name } -Descending
$makeappx = $sdk | ForEach-Object { "$($_.FullName)\$machine\makeappx.exe", "$($_.FullName)\x64\makeappx.exe" } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $makeappx) { throw 'MakeAppx is not in any Windows SDK here' }

$package = Join-Path (Resolve-Path $Output).Path "Lumenna-$Version-$Arch.msix"
& $makeappx pack /o /d $layout /p $package
if ($LASTEXITCODE -ne 0) { throw 'MakeAppx did not pack the package' }
Write-Output $package
