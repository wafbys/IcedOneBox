# 端到端冒烟：构建核 -> 生成头文件 -> 编译并运行 C 测试。
#
# 注意：这里刻意不用 $ErrorActionPreference = "Stop"。cargo 会把进度写到
# stderr，依赖 PowerShell 的隐式行为会在管道场景（如 | Select-String）下把
# stderr 误判为终止错误。改为显式检查每个外部命令的 $LASTEXITCODE。
param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "debug"
)

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
if ($LASTEXITCODE -ne 0) {
    throw "cargo build 失败（退出码 $LASTEXITCODE）"
}

& (Join-Path $repoRoot "examples\c-smoke\build.ps1") -Profile $Profile
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

& (Join-Path $repoRoot "examples\c-smoke\c-smoke.exe")
exit $LASTEXITCODE
