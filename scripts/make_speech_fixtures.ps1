param([string]$OutputDir = "$PSScriptRoot\..\test-artifacts\speech")
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Speech
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
$cases = @(
    @{ Id = 'chinese'; Voice = 'Microsoft Huihui Desktop'; Text = '今天我们测试本地语音输入。请把这段文字准确地输入到文本框中。' },
    @{ Id = 'english'; Voice = 'Microsoft Zira Desktop'; Text = 'This is a local speech recognition test. Please type this sentence into the text box.' }
)
foreach ($case in $cases) {
    $speaker = New-Object System.Speech.Synthesis.SpeechSynthesizer
    try {
        $speaker.SelectVoice($case.Voice)
        $speaker.Rate = -1
        $speaker.SetOutputToWaveFile((Join-Path $OutputDir ($case.Id + '.wav')))
        $speaker.Speak($case.Text)
    } finally { $speaker.Dispose() }
}
$cases | ConvertTo-Json | Set-Content (Join-Path $OutputDir 'expected.json') -Encoding UTF8
Write-Output "Speech fixtures created: $OutputDir"
