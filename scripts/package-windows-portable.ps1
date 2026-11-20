#Requires -Version 5.1
param([Parameter(Mandatory)][string]$Executable)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$exe = Get-Item -LiteralPath $Executable
if ($exe.PSIsContainer -or $exe.Name -notin @('grit-work.exe', 'Work_Review.exe')) {
    throw 'Expected the built grit-work.exe or Work_Review.exe.'
}
$archive = Join-Path $exe.DirectoryName ($exe.BaseName + '_portable_x64.zip')
$staging = Join-Path ([IO.Path]::GetTempPath()) ('work-review-portable-package-' + [guid]::NewGuid().ToString('N'))
$temporaryArchive = $staging + '.zip'
[void][IO.Directory]::CreateDirectory($staging)
try {
    # 只复制构建的 EXE，绝不打包程序旁已有的用户 Data。
    Copy-Item -LiteralPath $exe.FullName -Destination (Join-Path $staging $exe.Name)
    foreach ($name in @('install-ocr-languages.ps1', 'setup-ocr.cmd')) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot ('..\src-tauri\windows\' + $name)) -Destination (Join-Path $staging $name)
    }
    [void][IO.Directory]::CreateDirectory((Join-Path $staging 'Data'))
    $readme = @'
GRIT Work OCR setup / OCR-språk

Kör setup-ocr.cmd före första starten på en ny dator. Svenska (sv-SE) och engelska
(en-US) installeras via Windows Update. Saknade språk kräver internet och
administratörsgodkännande. Starta om Windows om installationen begär det.
Dina valda visningsspråk och tangentbord ändras inte. OCR-språken delas med andra appar
och tas inte bort när den här portabla mappen tas bort.

Run setup-ocr.cmd before first use on a new PC. It installs Swedish (sv-SE) and
English (en-US) OCR through Windows Update. Missing languages need internet and
administrator approval. Restart Windows if requested. Existing OCR languages are
reused; display language and keyboard settings are unchanged. These Windows
components remain installed when the portable folder is removed.

Work Review 便携版 / Portable edition

- 解压整个 ZIP 到可写目录，双击 Work_Review.exe。需要 Microsoft Edge WebView2 Runtime。
- Data 目录是便携模式标记：数据库、配置、截图及更新设置保存在这里，优先于本机的自定义数据目录。
- 不会自动导入、覆盖或删除原有数据；现有用户请先备份，并在完全退出后手动复制完整数据目录到 Data。
- 更换位置或电脑前完全退出程序，再一起移动 EXE 和 Data；配置可能含 API Key，请勿公开分享 Data。
- Data 不可读写时停止启动并提示，不会悄悄改用其他目录。
- 便携模式只检查更新并打开发布页，请下载新版 ZIP，退出后只替换 EXE，保留原 Data；不要使用安装版 EXE 升级。
- 移走 Data 并重启可退出便携模式，重新使用本机原有数据目录；不会自动把便携数据搬回去。
- WebView2 缓存、自启动注册及用户选择的外部导出路径仍由系统管理，不承诺零系统痕迹。

Extract the entire ZIP to a writable folder; keep Work_Review.exe and Data together.
Data opts into portable mode and takes priority over this computer's saved data location.
Back up first. No existing data is automatically imported, overwritten or deleted.
Exit the app before moving the folder or copying your full data directory into Data.
Data may contain API keys: never share it publicly. An unwritable Data directory stops startup.
For updates, download the new portable ZIP and replace the EXE after exiting; preserve your Data.
Do not use the installer to upgrade the portable app. Removing Data restores this PC's normal mode.
WebView2 cache, autostart registration and external export destinations remain system-managed.
'@
    [IO.File]::WriteAllText((Join-Path $staging 'PORTABLE_README.txt'), $readme.Replace('Work_Review.exe', $exe.Name), [Text.UTF8Encoding]::new($false))
    [IO.Compression.ZipFile]::CreateFromDirectory($staging, $temporaryArchive)
    $zip = [IO.Compression.ZipFile]::Open($temporaryArchive, [IO.Compression.ZipArchiveMode]::Update)
    try {
        # Windows PowerShell's .NET Framework may emit a backslash for empty directories.
        $legacyDirectory = $zip.GetEntry('Data\')
        if ($null -ne $legacyDirectory) {
            $legacyDirectory.Delete()
            [void]$zip.CreateEntry('Data/')
        }
        $entries = @($zip.Entries | ForEach-Object FullName | Sort-Object)
        $expected = @('Data/', 'PORTABLE_README.txt', $exe.Name, 'install-ocr-languages.ps1', 'setup-ocr.cmd') | Sort-Object
        if (@(Compare-Object $expected $entries).Count -ne 0) {
            throw ('Portable archive entries do not match: ' + ($entries -join ', '))
        }
        if ($zip.GetEntry($exe.Name).Length -ne $exe.Length) {
            throw 'Portable executable size did not verify.'
        }
    } finally {
        $zip.Dispose()
    }
    Move-Item -LiteralPath $temporaryArchive -Destination $archive -Force
    Write-Output ('Portable archive verified: ' + $archive)
} finally {
    Remove-Item -LiteralPath $staging -Recurse -Force
    if (Test-Path -LiteralPath $temporaryArchive) {
        Remove-Item -LiteralPath $temporaryArchive -Force
    }
}
