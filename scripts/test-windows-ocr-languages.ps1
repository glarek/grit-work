#Requires -Version 5.1
# Exercise the prerequisite workflow without changing Windows or requiring Pester.
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\src-tauri\windows\install-ocr-languages.ps1')

function Reset-Scenario {
    $script:Missing = @('sv-SE', 'en-US')
    $script:Administrator = $true
    $script:Installed = @()
    $script:Reads = 0
    $script:Elevations = 0
    $script:ElevatedExit = 0
    $script:Failure = $false
    $script:Restart = $false
    $script:AvailableAfterInstall = $true
}
function Get-GritMissingOcrLanguages { $script:Missing }
function Test-GritAdministrator { $script:Administrator }
function Invoke-GritOcrElevation {
    $script:Elevations++
    $script:ElevatedExit
}
function Get-WindowsCapability {
    [CmdletBinding()]
    param([switch]$Online, [string]$Name)
    $script:Reads++
    [pscustomobject]@{ Name = $Name; State = 'NotPresent' }
}
function Add-WindowsCapability {
    [CmdletBinding()]
    param([switch]$Online, [string]$Name)
    if ($script:Failure) { throw 'Synthetic Windows Update failure' }
    $script:Installed += $Name
    $tag = $Name.Split('~')[3]
    if ($script:AvailableAfterInstall) { $script:Missing = @($script:Missing | Where-Object { $_ -ne $tag }) }
    [pscustomobject]@{ RestartNeeded = $script:Restart }
}
function Assert-Scenario([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}

Reset-Scenario
$script:Missing = @(); $script:Administrator = $false
Assert-Scenario ((Install-GritOcrLanguages) -eq 0 -and $script:Reads -eq 0 -and $script:Elevations -eq 0) 'Existing languages must work without admin or Windows Update.'

Reset-Scenario
Assert-Scenario ((Install-GritOcrLanguages -CheckOnly) -eq 2 -and $script:Reads -eq 0) 'Check-only must never install.'

Reset-Scenario
Assert-Scenario ((Install-GritOcrLanguages) -eq 0) 'Both missing languages must install.'
Assert-Scenario (($script:Installed -join ',') -eq 'Language.OCR~~~sv-SE~0.0.1.0,Language.OCR~~~en-US~0.0.1.0') 'Must install exactly Swedish and English OCR.'

Reset-Scenario
$script:Missing = @('sv-SE')
Assert-Scenario ((Install-GritOcrLanguages) -eq 0 -and $script:Installed.Count -eq 1) 'Already installed English must remain untouched.'

Reset-Scenario
$script:Administrator = $false
Assert-Scenario ((Install-GritOcrLanguages -NoElevation) -eq 740 -and $script:Elevations -eq 0) 'Silent missing prerequisites must fail without prompting.'

Reset-Scenario
$script:Administrator = $false; $script:ElevatedExit = 1223
Assert-Scenario ((Install-GritOcrLanguages) -eq 1223 -and $script:Elevations -eq 1) 'Cancelled elevation must not claim success.'

Reset-Scenario
$script:Failure = $true
Assert-Scenario ((Install-GritOcrLanguages) -eq 1) 'Windows Update errors must block installation.'

Reset-Scenario
$script:Restart = $true; $script:AvailableAfterInstall = $false
Assert-Scenario ((Install-GritOcrLanguages) -eq 3010) 'Restart requirement must reach the installer.'

Reset-Scenario
$script:AvailableAfterInstall = $false
Assert-Scenario ((Install-GritOcrLanguages) -eq 1) 'Capability success without registered recognizers must fail.'

Write-Output 'PASS: 9 OCR prerequisite scenarios. No system capabilities were changed by this test.'
