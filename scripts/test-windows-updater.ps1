# Exercise the real NSIS installer against disposable payloads and registry keys.
$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$compiler = Get-Command makensis -ErrorAction SilentlyContinue
if ($compiler) { $makensis = $compiler.Source }
else { $makensis = "C:\Program Files (x86)\NSIS\makensis.exe" }
if (-not (Test-Path $makensis)) { throw "NSIS is required for updater tests" }
$name = "ImmersionUpdaterTest-" + [guid]::NewGuid().ToString("N")
$temp = Join-Path ([IO.Path]::GetTempPath()) $name
$install = Join-Path $temp "install path with spaces"
$runKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
New-Item -ItemType Directory -Path $temp | Out-Null
$locked = $null
try {
    foreach ($version in @("0.0.1", "0.0.2")) {
        $source = Join-Path $temp "$version.exe"
        [IO.File]::WriteAllText($source, $version)
        & $makensis "/DVERSION=$version" "/DAPP_NAME=$name" "/DEXE_SOURCE=$source" `
            "/DLICENSE_SOURCE=$(Join-Path $root 'LICENSE')" "/DOUTFILE=$(Join-Path $temp "setup-$version.exe")" `
            (Join-Path $root "packaging\nsis\Immersion.nsi")
        if ($LASTEXITCODE -ne 0) { throw "NSIS compilation failed" }
    }
    function RunInstaller($version) {
        (Start-Process -FilePath (Join-Path $temp "setup-$version.exe") `
            -ArgumentList "/S /D=$install" -Wait -PassThru).ExitCode
    }
    if ((RunInstaller "0.0.1") -ne 0) { throw "Initial install failed" }
    $exe = Join-Path $install "Immersion.exe"
    New-ItemProperty -Path $runKey -Name $name -Value "preserved startup preference" -Force | Out-Null
    $locked = [IO.File]::Open($exe, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::None)
    if ((RunInstaller "0.0.2") -eq 0) { throw "Locked executable replacement unexpectedly succeeded" }
    $locked.Dispose()
    $locked = $null
    if ([IO.File]::ReadAllText($exe) -ne "0.0.1") { throw "Failed upgrade damaged original" }
    if ((RunInstaller "0.0.2") -ne 0) { throw "Upgrade failed" }
    if ([IO.File]::ReadAllText($exe) -ne "0.0.2") { throw "Wrong installed binary" }
    if ([IO.File]::ReadAllText((Join-Path $install "Immersion.previous.exe")) -ne "0.0.1") {
        throw "Missing recovery backup"
    }
    $startup = Get-ItemPropertyValue -Path $runKey -Name $name
    if ($startup -ne "preserved startup preference") { throw "Upgrade changed startup preference" }
    $recorded = Get-ItemPropertyValue -Path "HKCU:\Software\$name" -Name InstallDir
    if ($recorded -ne $install) { throw "Upgrade changed install location" }
    $version = Get-ItemPropertyValue -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$name" -Name DisplayVersion
    if ($version -ne "0.0.2") { throw "Upgrade did not update uninstall metadata" }
    Write-Host "Updater installer tests passed"
}
finally {
    if ($locked) { $locked.Dispose() }
    if (Test-Path (Join-Path $install "Uninstall.exe")) {
        Start-Process -FilePath (Join-Path $install "Uninstall.exe") -ArgumentList "/S _?=$install" -Wait
    }
    Remove-ItemProperty -Path $runKey -Name $name -ErrorAction SilentlyContinue
    Remove-Item -Path "HKCU:\Software\$name" -Recurse -ErrorAction SilentlyContinue
    Remove-Item -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$name" -Recurse -ErrorAction SilentlyContinue
    Remove-Item -Path $temp -Recurse -Force -ErrorAction SilentlyContinue
}
