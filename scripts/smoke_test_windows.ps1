<#
.SYNOPSIS
    Automated Windows smoke test suite for LavaTerm (PowerShell 5.1 & 7+).
#>
param(
    [string]$BinaryPath = "",
    [switch]$RunCargoTests
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($BinaryPath)) {
    $candidates = @("target\release\lavaterm.exe", "target\debug\lavaterm.exe")
    $BinaryPath = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $BinaryPath) {
        Write-Host "[FAIL] No lavaterm.exe found. Run 'cargo build' or 'cargo build --release' first." -ForegroundColor Red
        exit 1
    }
} elseif (-not (Test-Path $BinaryPath)) {
    Write-Host "[FAIL] Binary not found at '$BinaryPath'." -ForegroundColor Red
    exit 1
}

if ($RunCargoTests) {
    Write-Host "=== Running cargo fmt, clippy, and unit/integration tests ===" -ForegroundColor Cyan
    cargo fmt --check
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    cargo clippy --all-targets --all-features -- -D warnings
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    cargo test
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$script:Passed = 0
$script:Failed = 0

function Invoke-Lavaterm {
    param([string[]]$CliArgs)
    $prevEncoding = [Console]::OutputEncoding
    [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
    try {
        $prevEap = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        $lines = & $BinaryPath @CliArgs 2>&1
        $code = $LASTEXITCODE
        $ErrorActionPreference = $prevEap
        $text = ($lines | ForEach-Object { "$_" }) -join "`n"
        return @{ Code = $code; Text = $text }
    } finally {
        [Console]::OutputEncoding = $prevEncoding
    }
}

function Test-Pass {
    param([string]$Name, [string[]]$CliArgs, [string]$ExpectedPattern = "", [switch]$SingleLine)
    $res = Invoke-Lavaterm -CliArgs $CliArgs
    $ok = ($res.Code -eq 0)
    if ($ok -and $ExpectedPattern -and ($res.Text -notmatch $ExpectedPattern)) { $ok = $false }
    if ($ok -and $SingleLine -and ($res.Text.Trim() -match "`n")) { $ok = $false }

    if ($ok) {
        Write-Host "[PASS] $Name" -ForegroundColor Green
        $script:Passed++
    } else {
        Write-Host "[FAIL] $Name (exit=$($res.Code))" -ForegroundColor Red
        if ($res.Text) { Write-Host "       Output: $($res.Text.Substring(0, [Math]::Min(200, $res.Text.Length)))" }
        $script:Failed++
    }
}

function Test-Fail {
    param([string]$Name, [string[]]$CliArgs)
    $res = Invoke-Lavaterm -CliArgs $CliArgs
    $ok = ($res.Code -ne 0) -and ($res.Text -notmatch "panicked at")
    if ($ok) {
        Write-Host "[PASS] $Name (rejected cleanly)" -ForegroundColor Green
        $script:Passed++
    } else {
        Write-Host "[FAIL] $Name (exit=$($res.Code))" -ForegroundColor Red
        if ($res.Text) { Write-Host "       Output: $($res.Text.Substring(0, [Math]::Min(200, $res.Text.Length)))" }
        $script:Failed++
    }
}

Write-Host "=== LavaTerm Windows Smoke Tests ($BinaryPath) ===" -ForegroundColor Cyan

# 1. Metadata
Test-Pass "Metadata: --version" @("--version") "lavaterm"
Test-Pass "Metadata: --help" @("--help") "Usage:"

# 2. Headless modes
Test-Pass "Headless: standard" @("--headless", "--frames", "20") "Headless simulation completed successfully"
Test-Pass "Headless: --system" @("--headless", "--frames", "20", "--system") "system=true"
Test-Pass "Headless: --audio" @("--headless", "--frames", "20", "--audio") "audio=true"
Test-Pass "Headless: --audio --audio-loopback" @("--headless", "--frames", "20", "--audio", "--audio-loopback") "audio=true"
Test-Pass "Headless: --audio-loopback auto-enables audio" @("--headless", "--frames", "10", "--audio-loopback") "audio=true"

# 3. Snapshots (all 3 renderers + 1-row micro geometry)
Test-Pass "Snapshot: halfblock 40x8" @("--snapshot", "--width", "40", "--height", "8", "--renderer", "halfblock", "--theme", "lava") "\x1b\[0m"
Test-Pass "Snapshot: block 40x8" @("--snapshot", "--width", "40", "--height", "8", "--renderer", "block", "--theme", "matrix") "\x1b\[38;2;"
Test-Pass "Snapshot: braille 40x8" @("--snapshot", "--width", "40", "--height", "8", "--renderer", "braille", "--theme", "cyberpunk") "\x1b\[38;2;"
Test-Pass "Snapshot: single-row braille 20x1" @("--snapshot", "--width", "20", "--height", "1", "--renderer", "braille") "\x1b\[38;2;" -SingleLine

# 4. Audio device enumeration
Test-Pass "Audio: --list-audio-devices" @("--list-audio-devices") "Available Audio Capture Devices"

# 5. CLI validation negative checks
Test-Fail "Reject: --blobs 0" @("--blobs", "0")
Test-Fail "Reject: --blobs 500" @("--blobs", "500")
Test-Fail "Reject: --fps 0" @("--fps", "0")
Test-Fail "Reject: --fps 999" @("--fps", "999")
Test-Fail "Reject: --width 5000 --height 5000" @("--width", "5000", "--height", "5000")
Test-Fail "Reject: --renderer bogus" @("--renderer", "bogus")
Test-Fail "Reject: --shockwave-force NaN" @("--shockwave-force", "NaN")
Test-Fail "Reject: --system --audio conflict" @("--system", "--audio")
Test-Fail "Reject: --system --audio-loopback conflict" @("--system", "--audio-loopback")
Test-Fail "Reject: nonexistent --audio-device fails fast" @("--headless", "--frames", "5", "--audio-device", "__LavaTerm_NonExistent_Device__")

# 6. Theme & config parser regression checks
$brokenJson = Join-Path $env:TEMP "lavaterm_broken_$PID.json"
$utf8HexJson = Join-Path $env:TEMP "lavaterm_utf8_$PID.json"
$unknownKeyToml = Join-Path $env:TEMP "lavaterm_unknown_$PID.toml"
try {
    [System.IO.File]::WriteAllText($brokenJson, '{ "bottom": "#ff0000"', [System.Text.UTF8Encoding]::new($false))
    [System.IO.File]::WriteAllText($utf8HexJson, '{ "bottom": "#ééé" }', [System.Text.UTF8Encoding]::new($false))
    [System.IO.File]::WriteAllText($unknownKeyToml, "[render]`nglo = true`n", [System.Text.UTF8Encoding]::new($false))

    Test-Fail "Theme: broken JSON returns error" @("--theme", $brokenJson, "--headless", "--frames", "1")
    Test-Fail "Theme: 6-byte UTF-8 hex returns error without panic" @("--theme", $utf8HexJson, "--headless", "--frames", "1")
    Test-Fail "Config: unknown TOML key rejected cleanly" @("--config", $unknownKeyToml, "--headless", "--frames", "1")
} finally {
    Remove-Item -Path $brokenJson, $utf8HexJson, $unknownKeyToml -Force -ErrorAction SilentlyContinue
}

$total = $script:Passed + $script:Failed
Write-Host "`nSummary: $($script:Passed)/$total passed." -ForegroundColor $(if ($script:Failed -eq 0) { "Green" } else { "Red" })
if ($script:Failed -gt 0) { exit 1 } else { exit 0 }
