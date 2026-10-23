# Installs a packed MSIX on this machine and checks it, then takes it away again: for CI's
# runners, which are thrown away after. It signs a copy with a certificate made for the run and
# trusts that certificate machine-wide, since Windows installs no unsigned package; never run it
# on a computer someone uses.
#
#   powershell -File apps\windows\installer\msix\test.ps1 -Package dist\Lumenna-0.1.0-x64.msix
param(
    [Parameter(Mandatory)] [string] $Package,
    [string] $IdentityName = 'dev.emassey0135.Lumenna',
    [string] $Publisher = 'CN=Elijah Massey'
)
$ErrorActionPreference = 'Stop'

$machine = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x64' }
$sdk = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.*" -Directory | Sort-Object { [version]$_.Name } -Descending
$signtool = $sdk | ForEach-Object { "$($_.FullName)\$machine\signtool.exe", "$($_.FullName)\x64\signtool.exe" } | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $signtool) { throw 'SignTool is not in any Windows SDK here' }

# A code-signing certificate whose subject is the package's publisher, as Windows requires.
$cert = New-SelfSignedCertificate -Type Custom -Subject $Publisher -KeyUsage DigitalSignature -FriendlyName 'Lumenna CI' `
    -CertStoreLocation Cert:\CurrentUser\My -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')
$work = Join-Path ([IO.Path]::GetTempPath()) 'lumenna-msix-test'
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $work | Out-Null
$signed = "$work\signed.msix"
Copy-Item $Package $signed
& $signtool sign /fd SHA256 /sha1 $cert.Thumbprint $signed
if ($LASTEXITCODE -ne 0) { throw 'SignTool did not sign the test copy' }
Export-Certificate -Cert $cert -FilePath "$work\ci.cer" | Out-Null
Import-Certificate -FilePath "$work\ci.cer" -CertStoreLocation Cert:\LocalMachine\TrustedPeople | Out-Null

try {
    Add-AppxPackage -Path $signed
    $installed = Get-AppxPackage -Name $IdentityName
    if (-not $installed) { throw "the package $IdentityName did not install" }
    Write-Output "Installed $($installed.PackageFullName)"

    # `lum` by its alias, in the package: it runs, and keeps its store in the package's own folder.
    $lum = "$env:LOCALAPPDATA\Microsoft\WindowsApps\lum.exe"
    if (-not (Test-Path $lum)) { throw 'lum has no alias' }
    & $lum --version
    if ($LASTEXITCODE -ne 0) { throw 'lum did not run through its alias' }
    $ordinary = "$env:LOCALAPPDATA\lumenna\data\lumenna.sqlite"
    $before = Test-Path $ordinary
    & $lum task add 'Made in the package'
    if ($LASTEXITCODE -ne 0) { throw 'lum could not add a task' }
    $store = "$env:LOCALAPPDATA\Packages\$($installed.PackageFamilyName)\LocalCache\Local\lumenna\data\lumenna.sqlite"
    if (-not (Test-Path $store)) { throw "the package's store is not at $store" }
    if (-not $before -and (Test-Path $ordinary)) { throw "the package wrote the ordinary install's store, $ordinary" }

    # Opening at sign-in is the package's startup task, off until the person turns it on.
    $manifest = [xml](Get-Content -Raw "$($installed.InstallLocation)\AppxManifest.xml")
    if ($manifest.OuterXml -notmatch 'windows\.startupTask') { throw 'the package declares no startup task' }

    Remove-AppxPackage -Package $installed.PackageFullName
    if (Get-AppxPackage -Name $IdentityName) { throw 'the package did not uninstall' }
    Write-Output 'Installed, ran, kept its own store, and uninstalled'
}
finally {
    Get-ChildItem Cert:\LocalMachine\TrustedPeople | Where-Object Thumbprint -eq $cert.Thumbprint | Remove-Item
    Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)"
}
