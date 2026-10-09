# 端到端冒烟：构建核 -> 生成头文件 -> 编译并运行 C 测试。
param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "debug"
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

Push-Location $repoRoot
try {
    if ($Profile -eq "release") {
        cargo build --release
    }
    else {
        cargo build
    }
}
finally {
    Pop-Location
}

& (Join-Path $repoRoot "examples\c-smoke\build.ps1") -Profile $Profile
& (Join-Path $repoRoot "examples\c-smoke\c-smoke.exe")
exit $LASTEXITCODE
