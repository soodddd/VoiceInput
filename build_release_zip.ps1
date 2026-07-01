# ============================================================
#  VoiceInput v2 — 预打包 zip 版本构建脚本
#
#  将 voiceinput.exe + asr_backend.exe + 资源文件打包为
#  解压即用的 zip 分发包。绕过 NSIS 对超大 sidecar 的 mmap 限制。
#
#  用法: powershell -ExecutionPolicy Bypass -File build_release_zip.ps1
# ============================================================

param(
    [string]$Version = "0.1.0-preview",
    [string]$OutputDir = ".\release"
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "=========================================="
Write-Host " VoiceInput v$Version - Release Zip Builder"
Write-Host "=========================================="

# ── 1. 验证构建产物存在 ──
Write-Host "`n[1/5] 验证构建产物..."

$VoiceinputExe = Join-Path $ProjectRoot "src-tauri\target\release\voiceinput.exe"
$SidecarExe = Join-Path $ProjectRoot "src-tauri\binaries\asr_backend.exe"
$DefaultConfig = Join-Path $ProjectRoot "resources\default_config.json"
$IconFile = Join-Path $ProjectRoot "src-tauri\resources\icon.ico"

$RequiredFiles = @(
    @{ Path = $VoiceinputExe; Desc = "Tauri 主程序" },
    @{ Path = $SidecarExe; Desc = "Python ASR sidecar" },
    @{ Path = $DefaultConfig; Desc = "默认配置文件" },
    @{ Path = $IconFile; Desc = "应用图标" }
)

foreach ($f in $RequiredFiles) {
    if (-not (Test-Path $f.Path)) {
        Write-Host "ERROR: $($f.Desc) 不存在: $($f.Path)" -ForegroundColor Red
        Write-Host "请先运行: npm run tauri build (生成 voiceinput.exe) 和 build_backend.bat (生成 asr_backend.exe)"
        exit 1
    }
    $size = (Get-Item $f.Path).Length / 1MB
    Write-Host "  OK: $($f.Desc) ($('{0:N1}' -f $size) MB)"
}

# ── 2. 创建临时打包目录 ──
Write-Host "`n[2/5] 准备打包目录..."

$StagingDir = Join-Path $env:TEMP "voiceinput_release_$Version"
if (Test-Path $StagingDir) {
    Remove-Item -Recurse -Force $StagingDir
}
$BinariesDir = Join-Path $StagingDir "binaries"
$ResourcesDir = Join-Path $StagingDir "resources"
New-Item -ItemType Directory -Path $StagingDir | Out-Null
New-Item -ItemType Directory -Path $BinariesDir | Out-Null
New-Item -ItemType Directory -Path $ResourcesDir | Out-Null

# ── 3. 复制文件 ──
Write-Host "`n[3/5] 复制文件到打包目录..."

Copy-Item $VoiceinputExe -Destination $StagingDir -Force
Write-Host "  voiceinput.exe -> 根目录"

Copy-Item $SidecarExe -Destination $BinariesDir -Force
Write-Host "  asr_backend.exe -> binaries/"

Copy-Item $DefaultConfig -Destination $ResourcesDir -Force
Write-Host "  default_config.json -> resources/"

Copy-Item $IconFile -Destination $ResourcesDir -Force
Write-Host "  icon.ico -> resources/"

# 生成 README.txt
$ReadmeContent = @"
VoiceInput v$Version - Windows 本地语音输入法
================================================

【系统要求】
- Windows 10 1903+ 或 Windows 11
- NVIDIA GPU（CUDA 11.8+ 兼容）
- 4GB+ 显存
- 200MB 磁盘空间（不含模型）
- 麦克风

【快速开始】
1. 解压此 zip 到任意目录（如 C:\Program Files\VoiceInput\）
2. 双击运行 voiceinput.exe
3. 首次启动会提示下载语音识别模型（约 1.2 GB）
4. 模型加载完成后，按 Alt+V 开始语音输入

【快捷键】
- Alt+V：按住说话，松开后自动识别并粘贴
- Alt+L：切换识别语言（Auto / 中文 / 英文）

【文件结构】
voiceinput.exe              主程序
binaries\asr_backend.exe    Python ASR 后端（本地运行）
resources\                  默认配置和图标
%LOCALAPPDATA%\VoiceInput\  用户配置和模型存储目录

【隐私说明】
所有语音识别在本地 GPU 完成，不上传任何数据到云端。

【技术支持】
日志文件位于: %LOCALAPPDATA%\VoiceInput\logs\
"@

$ReadmePath = Join-Path $StagingDir "README.txt"
Set-Content -Path $ReadmePath -Value $ReadmeContent -Encoding UTF8
Write-Host "  README.txt -> 根目录"

# ── 4. 创建 zip ──
Write-Host "`n[4/5] 创建 zip 分发包..."

if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

$ZipName = "VoiceInput-v$Version-win64-preview.zip"
$ZipPath = Join-Path $OutputDir $ZipName

if (Test-Path $ZipPath) {
    Remove-Item $ZipPath -Force
}

# 使用 .NET ZipFile 类（支持大文件）
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($StagingDir, $ZipPath, [System.IO.Compression.CompressionLevel]::Optimal, $false)

$ZipSize = (Get-Item $ZipPath).Length / 1MB
Write-Host "  已创建: $ZipPath"
Write-Host "  大小: $('{0:N1}' -f $ZipSize) MB"

# ── 5. 清理临时目录 ──
Write-Host "`n[5/5] 清理临时文件..."
Remove-Item -Recurse -Force $StagingDir
Write-Host "  已清理临时打包目录"

# ── 完成 ──
Write-Host "`n=========================================="
Write-Host " 构建完成！"
Write-Host "=========================================="
Write-Host "分发包: $ZipPath"
Write-Host "大小: $('{0:N1}' -f $ZipSize) MB"
Write-Host ""
Write-Host "用户使用方式: 解压 zip 到任意目录，运行 voiceinput.exe"
Write-Host ""
