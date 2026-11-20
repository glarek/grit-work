#Requires -Version 5.1
<#
Ensure the two Windows.Media.Ocr dependencies before installing GRIT Work.
Windows Update supplies packages matching this Windows build and architecture.
No display language, keyboard layout or user language preference is changed.
Exit codes: 0 ready, 2 missing (check only), 3010 restart required,
740 elevation required, 1223 elevation cancelled, 1 installation failed.
Dot-source this file to test the functions without installing anything.
#>
[CmdletBinding()]
param([switch]$CheckOnly, [switch]$NoElevation)

$script:GritOcrScriptPath = $PSCommandPath

function Get-GritMissingOcrLanguages {
    [Windows.Media.Ocr.OcrEngine, Windows.Foundation, ContentType = WindowsRuntime] | Out-Null
    $available = @([Windows.Media.Ocr.OcrEngine]::AvailableRecognizerLanguages | ForEach-Object LanguageTag)
    @('sv-SE', 'en-US') | Where-Object { $_ -notin $available }
}

function Test-GritAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Invoke-GritOcrElevation {
    $powershell = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
    if ([Environment]::Is64BitOperatingSystem -and -not [Environment]::Is64BitProcess) {
        $powershell = Join-Path $env:WINDIR 'Sysnative\WindowsPowerShell\v1.0\powershell.exe'
    }
    $arguments = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + $script:GritOcrScriptPath + '" -NoElevation'
    try {
        $process = Start-Process -FilePath $powershell -ArgumentList $arguments -Verb RunAs -PassThru
        $null = $process.Handle
        # Wait for this process only; Start-Process -Wait also waits for descendants.
        $process.WaitForExit()
        if ($null -eq $process.ExitCode) {
            Write-Host 'Could not verify the elevated OCR setup result.'
            return 1
        }
        return [int]$process.ExitCode
    } catch {
        Write-Host 'Administrator approval for OCR language installation was cancelled or unavailable.'
        return 1223
    }
}

function Install-GritOcrLanguages {
    param([switch]$CheckOnly, [switch]$NoElevation)
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    try {
        $missing = @(Get-GritMissingOcrLanguages)
        if ($missing.Count -eq 0) {
            Write-Host 'OCR ready: sv-SE, en-US.'
            return 0
        }
        Write-Host ('Missing OCR languages: ' + ($missing -join ', '))
        if ($CheckOnly) { return 2 }
        if (-not (Test-GritAdministrator)) {
            if ($NoElevation) { return 740 }
            return (Invoke-GritOcrElevation)
        }

        $restart = $false
        foreach ($tag in $missing) {
            $name = 'Language.OCR~~~' + $tag + '~0.0.1.0'
            $capability = Get-WindowsCapability -Online -Name $name -ErrorAction Stop
            if (-not $capability -or $capability.Name -ne $name) {
                throw ('Windows does not provide the required OCR capability: ' + $tag)
            }
            if ($capability.State -ne 'Installed') {
                Write-Host ('Installing OCR language ' + $tag + ' from Windows Update. This can take several minutes.')
                $result = Add-WindowsCapability -Online -Name $name -ErrorAction Stop
                $restart = $restart -or [bool]$result.RestartNeeded
            }
        }
        $missing = @(Get-GritMissingOcrLanguages)
        if ($restart) {
            Write-Host 'Restart Windows to finish installing the OCR languages.'
            return 3010
        }
        if ($missing.Count -gt 0) {
            throw ('OCR recognizers are still unavailable: ' + ($missing -join ', '))
        }
        Write-Host 'OCR ready: sv-SE, en-US.'
        return 0
    } catch {
        Write-Host ('OCR language installation failed: ' + $_.Exception.Message)
        return 1
    }
}

if ($MyInvocation.InvocationName -ne '.') {
    exit (Install-GritOcrLanguages -CheckOnly:$CheckOnly -NoElevation:$NoElevation)
}
