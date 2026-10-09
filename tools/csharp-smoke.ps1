# 端到端 C# 冒烟：构建核 -> 运行 .NET 控制台（构建时自动复制 onebox.dll）。
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

$project = Join-Path $repoRoot "examples\csharp-smoke\csharp-smoke.csproj"
$configuration = if ($Profile -eq "release") { "Release" } else { "Debug" }

dotnet run --project $project -c $configuration
exit $LASTEXITCODE
