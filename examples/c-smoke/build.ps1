# 编译并链接 C 冒烟测试。
#
# 前置：先在仓库根执行 `cargo build`（或 `cargo build --release`）。
# 该命令会生成 target/<profile>/onebox.dll 与 include/onebox.h。
param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "debug"
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$includeDir = Join-Path $repoRoot "include"
$targetDir = Join-Path $repoRoot "target\$Profile"
$source = Join-Path $PSScriptRoot "main.c"
$exe = Join-Path $PSScriptRoot "c-smoke.exe"

if (-not (Test-Path (Join-Path $includeDir "onebox.h"))) {
    throw "未找到 include\onebox.h，请先在仓库根执行 cargo build"
}

# 优先使用 cdylib 的导入库，避免手动补齐 Windows 系统链接库。
$importLib = Get-ChildItem $targetDir -Filter "*.dll.lib" -ErrorAction SilentlyContinue | Select-Object -First 1
$staticLib = Join-Path $targetDir "onebox.lib"

if (Get-Command cl.exe -ErrorAction SilentlyContinue) {
    if ($importLib) {
        cl.exe /nologo /W3 /I "$includeDir" "$source" /Fe:"$exe" `
            /link /LIBPATH:"$targetDir" "$($importLib.Name)"
        Copy-Item (Join-Path $targetDir "onebox.dll") $PSScriptRoot -Force
    }
    elseif (Test-Path $staticLib) {
        # 静态库方式：需要显式补上 Rust 静态库依赖的 Windows 系统库。
        cl.exe /nologo /W3 /I "$includeDir" "$source" /Fe:"$exe" `
            /link /LIBPATH:"$targetDir" onebox.lib `
            kernel32.lib advapi32.lib ws2_32.lib userenv.lib bcrypt.lib ntdll.lib
    }
    else {
        throw "未找到可链接的 onebox 库（既无 *.dll.lib 也无 onebox.lib）"
    }
}
elseif (Get-Command gcc -ErrorAction SilentlyContinue) {
    gcc -Wall -I "$includeDir" "$source" -L "$targetDir" -lonebox -o "$exe"
    Copy-Item (Join-Path $targetDir "onebox.dll") $PSScriptRoot -Force -ErrorAction SilentlyContinue
}
else {
    throw "未找到 C 编译器（cl.exe 或 gcc）。请安装 VS Build Tools 或 MinGW。"
}

Write-Host "已生成 $exe"
