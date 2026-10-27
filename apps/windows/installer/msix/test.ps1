# Installs a packed MSIX on this machine and checks it, then takes it away again: for CI's
# runners, which are thrown away after. It signs a copy with a certificate made for the run and
# trusts that certificate machine-wide, since Windows installs no unsigned package; never run it
# on a computer someone uses.
#
#   powershell -File apps\windows\installer\msix\test.ps1 -Package dist\Lumenna-0.1.0-x64.msix
param(
    [Parameter(Mandatory)] [string] $Package,
    # As pack.ps1 was given them: empty, the workspace's stand-ins.
    [string] $IdentityName,
    [string] $Publisher
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\..\metadata.ps1"
$lumenna = Read-LumennaMetadata
if (-not $IdentityName) { $IdentityName = $lumenna.id }
if (-not $Publisher) { $Publisher = "CN=$($lumenna.publisher)" }

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

# The Windows App Runtime the package depends on for its widget, which the Store would install
# with it: taken from Microsoft's own NuGet package of it where the runner has none.
$runtime = 'Microsoft.WindowsAppRuntime.2'
$runtimeVersion = '2.5.1'
if (-not (Get-AppxPackage -Name $runtime | Where-Object { "$($_.Architecture)" -eq $machine })) {
    $ProgressPreference = 'SilentlyContinue'
    $nuget = "$work\runtime.zip"
    Invoke-WebRequest "https://api.nuget.org/v3-flatcontainer/microsoft.windowsappsdk.runtime/$runtimeVersion/microsoft.windowsappsdk.runtime.$runtimeVersion.nupkg" -OutFile $nuget
    Expand-Archive $nuget "$work\runtime"
    Add-AppxPackage -Path "$work\runtime\tools\MSIX\win10-$machine\$runtime.msix"
    Write-Output "Installed $runtime $runtimeVersion"
}

# The widget provider's COM class, as the manifest names it (`win/widgets`).
$widgetClass = '18D24380-6221-4EAF-975D-40086936D300'

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

    # The Widgets board's provider: the widget extension, its class registered with the package,
    # and the app started by COM as that class, as the board starts it.
    if ($manifest.OuterXml -notmatch 'com\.microsoft\.windows\.widgets') { throw 'the package declares no widget' }
    if (-not (Test-Path "Registry::HKEY_CLASSES_ROOT\PackagedCom\ClassIndex\{$widgetClass}\$($installed.PackageFullName)")) {
        throw "the widget provider's class {$widgetClass} is not registered with the package"
    }
    $provider = [Activator]::CreateInstance([Type]::GetTypeFromCLSID([guid]$widgetClass))
    if (-not $provider) { throw 'COM did not start the widget provider' }
    $serving = Get-CimInstance Win32_Process -Filter "Name = 'lumenna.exe'" | Where-Object { $_.CommandLine -match '--widgets' }
    if (-not $serving) { throw 'the widget provider is not lumenna.exe started with --widgets' }
    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($provider)
    $serving | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Write-Output 'The widget provider is registered, and COM starts it'

    Remove-AppxPackage -Package $installed.PackageFullName
    if (Get-AppxPackage -Name $IdentityName) { throw 'the package did not uninstall' }
    Write-Output 'Installed, ran, kept its own store, served its widget, and uninstalled'
}
finally {
    Get-ChildItem Cert:\LocalMachine\TrustedPeople | Where-Object Thumbprint -eq $cert.Thumbprint | Remove-Item
    Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)"
}
