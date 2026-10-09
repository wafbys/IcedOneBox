# 端到端 C# 冒烟：构建核 -> 运行 .NET 控制台（构建时自动复制 onebox.dll）。
#
# 同 smoke.ps1：不用 $ErrorActionPreference = "Stop"，显式检查 $LASTEXITCODE，
# 避免管道场景下把 cargo 的 stderr 误判为终止错误。
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

$project = Join-Path $repoRoot "examples\csharp-smoke\csharp-smoke.csproj"
$configuration = if ($Profile -eq "release") { "Release" } else { "Debug" }

dotnet run --project $project -c $configuration
exit $LASTEXITCODE
