#!/usr/bin/env python3
"""Run synthetic fixtures through the production Rust OCR on the local Windows VM.
Reproduce from the repository root (requires the configured grit-windows SSH alias):
    node scripts/generate-ocr-fixtures.mjs
    cargo build --manifest-path scripts/ocr-harness/Cargo.toml --target x86_64-pc-windows-gnu --release
    python3 scripts/run-windows-ocr-corpus.py
    python3 scripts/summarize-ocr-corpus.py
The browser text round-trip suite runs separately:
    node scripts/with-grit-preview.mjs scripts/test-ocr-text-ui.mjs
Only the dedicated QA directory is written. No app data, credentials or live captures.
"""
import argparse
import base64
import json
from pathlib import Path
import subprocess
import zipfile

root = Path(__file__).resolve().parents[1]
artifacts = root / 'all-artifacts/ocr-robustness'
remote = 'C:/GritWorkReview/ocr-robustness-20261120'
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--language', choices=['auto', 'sv-SE', 'en-US'], default='auto')
language = parser.parse_args().language
suffix = '' if language == 'auto' else '-' + language

def ps(script):
    command = base64.b64encode(script.encode('utf-16le')).decode('ascii')
    subprocess.run(['ssh', '-o', 'ConnectTimeout=10', 'grit-windows', 'powershell -NoProfile -NonInteractive -EncodedCommand ' + command], check=True)

with zipfile.ZipFile(artifacts / 'fixtures.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
    for file in (artifacts / 'fixtures').iterdir():
        archive.write(file, file.name)
ps("$ErrorActionPreference='Stop';New-Item -ItemType Directory -Force -Path '" + remote + "' | Out-Null")
for source in [artifacts / 'fixtures.zip', root / 'scripts/ocr-harness/target/x86_64-pc-windows-gnu/release/grit-ocr-check.exe']:
    subprocess.run(['scp', str(source), 'grit-windows:' + remote + '/'], check=True)
ps(r'''
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
$OutputEncoding = [Console]::OutputEncoding
$root = 'C:\GritWorkReview\ocr-robustness-20261120'
$language = '__GRIT_TEST_LANGUAGE__'
$languageArgs = @()
if ($language -ne 'auto') { $languageArgs = @('--language', $language) }
Expand-Archive -Path "$root\fixtures.zip" -DestinationPath "$root\fixtures" -Force
$manifest = Get-Content "$root\fixtures\manifest.json" -Raw -Encoding UTF8 | ConvertFrom-Json
$results = @()
foreach ($case in $manifest) {
    $start = Get-Date
    # Native failures are expected for malformed input; record them as test outcomes.
    $ErrorActionPreference='Continue'
    $raw = & "$root\grit-ocr-check.exe" "$root\fixtures\$($case.file)" --json @languageArgs 2> "$root\last-stderr.txt"
    $code = $LASTEXITCODE
    $ErrorActionPreference='Stop'
    $decoded = $null
    if ($code -eq 0) { $decoded = ($raw -join "`n") | ConvertFrom-Json }
    $results += [pscustomobject]@{ id=$case.id; exit_code=$code; milliseconds=[int]((Get-Date)-$start).TotalMilliseconds; output=$decoded; error=([System.IO.File]::ReadAllText("$root\last-stderr.txt")) }
    Write-Output "$($case.id): exit=$code"
}
[System.IO.File]::WriteAllText("$root\results.json", (ConvertTo-Json -Depth 10 -InputObject $results), (New-Object System.Text.UTF8Encoding($false)))
'''.replace('__GRIT_TEST_LANGUAGE__', language))
destination = artifacts / ('windows-results' + suffix + '.json')
subprocess.run(['scp', 'grit-windows:' + remote + '/results.json', str(destination)], check=True)
print('Saved', destination)
