param(
    [string]$Version = "",
    [string]$OutputDir = ".\release"
)

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$TauriConfig = Get-Content (Join-Path $ProjectRoot "src-tauri\tauri.conf.json") -Raw |
    ConvertFrom-Json
if (-not $Version) {
    $Version = "$($TauriConfig.version)-preview"
}
$ExpectedAppVersion = $TauriConfig.version

$VoiceinputExe = Join-Path $ProjectRoot "src-tauri\target\release\voiceinput.exe"
$BackendDir = Join-Path $ProjectRoot "src-tauri\binaries\asr_backend"
$BackendExe = Join-Path $BackendDir "asr_backend.exe"
$DefaultConfig = Join-Path $ProjectRoot "resources\default_config.json"
$IconFile = Join-Path $ProjectRoot "src-tauri\resources\icon.ico"

$Required = @($VoiceinputExe, $BackendExe, $DefaultConfig, $IconFile)
foreach ($Path in $Required) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Required release input is missing: $Path"
    }
}
if (-not (Test-Path -LiteralPath (Join-Path $BackendDir "_internal") -PathType Container)) {
    throw "Backend _internal directory is missing. Rebuild with build_backend.bat."
}

$TempRoot = [System.IO.Path]::GetFullPath($env:TEMP)
$RunId = [Guid]::NewGuid().ToString("N")
$StagingDir = Join-Path $TempRoot "voiceinput-release-$RunId"
$ExtractDir = Join-Path $TempRoot "voiceinput-verify-$RunId"
$SmokeModelDir = Join-Path $TempRoot "voiceinput-model-$RunId"
$BackendProcess = $null

function Assert-SafeTempPath([string]$Path) {
    $Resolved = [System.IO.Path]::GetFullPath($Path)
    if (-not $Resolved.StartsWith($TempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to clean a path outside TEMP: $Resolved"
    }
}

try {
    Write-Host "[1/6] Staging release files..."
    New-Item -ItemType Directory -Path $StagingDir | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $StagingDir "resources") | Out-Null

    Copy-Item -LiteralPath $VoiceinputExe -Destination $StagingDir
    Copy-Item -LiteralPath $BackendDir -Destination $StagingDir -Recurse
    Copy-Item -LiteralPath $DefaultConfig -Destination (Join-Path $StagingDir "resources")
    Copy-Item -LiteralPath $IconFile -Destination (Join-Path $StagingDir "resources")

    Write-Host "[2/6] Writing manifest (large runtime; this may take a few minutes)..."
    $Readme = @"
VoiceInput v$Version - Windows 本地语音输入
=========================================

系统要求
- Windows 10 1903+ 或 Windows 11
- 支持 CUDA 的 NVIDIA GPU，建议 4 GB 以上显存
- 至少 6 GB 可用磁盘空间（程序约 2.7 GB，模型约 1.9 GB）
- 麦克风

使用方法
1. 必须完整解压 ZIP，不能直接在压缩包内运行。
2. 保持 asr_backend\_internal 与 asr_backend.exe 的相对位置不变。
3. 双击 voiceinput.exe。
4. 首次使用选择下载源或本地完整模型目录。
5. 点击麦克风开始/停止，或按住 Alt+V 说话并在松开后识别。

隐私
识别请求仅发送到本机随机回环端口。自动输入不修改剪贴板内容。

日志
%LOCALAPPDATA%\VoiceInput\logs\
"@
    Set-Content -LiteralPath (Join-Path $StagingDir "README.txt") -Value $Readme -Encoding UTF8

    $ManifestLines = Get-ChildItem -LiteralPath $StagingDir -File -Recurse |
        Sort-Object FullName |
        ForEach-Object {
            # Windows PowerShell 5.1/.NET Framework does not provide
            # Path.GetRelativePath; all files are descendants of StagingDir.
            $Relative = $_.FullName.Substring($StagingDir.Length).TrimStart('\', '/')
            $Hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
            "$Hash *$Relative"
        }
    Set-Content -LiteralPath (Join-Path $StagingDir "SHA256SUMS.txt") -Value $ManifestLines -Encoding UTF8

    $ResolvedOutput = if ([System.IO.Path]::IsPathRooted($OutputDir)) {
        $OutputDir
    } else {
        Join-Path $ProjectRoot $OutputDir
    }
    New-Item -ItemType Directory -Path $ResolvedOutput -Force | Out-Null
    $ZipPath = Join-Path $ResolvedOutput "VoiceInput-v$Version-win64.zip"
    if (Test-Path -LiteralPath $ZipPath) {
        Remove-Item -LiteralPath $ZipPath -Force
    }

    Write-Host "[3/6] Creating ZIP (large CUDA runtime; compression may take several minutes)..."
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [System.IO.Compression.ZipFile]::CreateFromDirectory(
        $StagingDir,
        $ZipPath,
        [System.IO.Compression.CompressionLevel]::Optimal,
        $false
    )

    Write-Host "[4/6] Extracting ZIP into a fresh verification directory..."
    New-Item -ItemType Directory -Path $ExtractDir | Out-Null
    [System.IO.Compression.ZipFile]::ExtractToDirectory($ZipPath, $ExtractDir)
    $ExtractedBackend = Join-Path $ExtractDir "asr_backend\asr_backend.exe"
    if (-not (Test-Path -LiteralPath $ExtractedBackend -PathType Leaf) -or
        -not (Test-Path -LiteralPath (Join-Path $ExtractDir "asr_backend\_internal") -PathType Container)) {
        throw "Fresh-extraction layout validation failed."
    }

    Write-Host "[5/6] Starting the freshly extracted backend for health smoke test..."
    New-Item -ItemType Directory -Path $SmokeModelDir | Out-Null
    $Listener = [System.Net.Sockets.TcpListener]::new(
        [System.Net.IPAddress]::Loopback,
        0
    )
    $Listener.Start()
    $SmokePort = ([System.Net.IPEndPoint]$Listener.LocalEndpoint).Port
    $Listener.Stop()
    $SmokeToken = [Guid]::NewGuid().ToString()
    $BackendProcess = Start-Process -FilePath $ExtractedBackend -ArgumentList @(
        "--token", $SmokeToken,
        "--port", "$SmokePort",
        "--model-dir", $SmokeModelDir,
        "--device", "cuda:0",
        "--model-strategy", "balanced"
    ) -WindowStyle Hidden -PassThru

    $Healthy = $false
    $Deadline = [DateTime]::UtcNow.AddSeconds(90)
    while ([DateTime]::UtcNow -lt $Deadline -and -not $BackendProcess.HasExited) {
        try {
            $Health = Invoke-RestMethod -Uri "http://127.0.0.1:$SmokePort/health" -TimeoutSec 2
            if ($Health.status -eq "ok" -and $Health.version -eq $ExpectedAppVersion) {
                $Healthy = $true
                break
            }
        } catch {
            Start-Sleep -Milliseconds 500
        }
    }
    if (-not $Healthy) {
        throw "Fresh-extraction backend health/version smoke test failed. Expected version $ExpectedAppVersion."
    }

    Write-Host "[6/6] Release verification complete."
    Write-Host "PASS: fresh extraction, _internal layout, manifest, and backend health"
    Write-Host "Release: $ZipPath"
    Write-Host ("Size: {0:N1} MB" -f ((Get-Item -LiteralPath $ZipPath).Length / 1MB))
}
finally {
    if ($null -ne $BackendProcess -and -not $BackendProcess.HasExited) {
        Stop-Process -Id $BackendProcess.Id -Force
        $BackendProcess.WaitForExit()
    }
    foreach ($Path in @($StagingDir, $ExtractDir, $SmokeModelDir)) {
        if (Test-Path -LiteralPath $Path) {
            Assert-SafeTempPath $Path
            Remove-Item -LiteralPath $Path -Recurse -Force
        }
    }
}
