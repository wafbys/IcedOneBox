# 编译并链接 C 冒烟测试。
#
# 前置：先在仓库根执行 `cargo build`（或 `cargo build --release`）。
# 该命令会生成 target/<profile>/onebox.dll 与 include/onebox.h。
param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "debug"
)

$ErrorActionPreference = "Stop"

# cl.exe 不在普通 PowerShell 的 PATH 里；这里通过 vswhere 自动加载 VS 开发环境。
function Enable-MsvcEnvironment {
    if (Get-Command cl.exe -ErrorAction SilentlyContinue) { return }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) { return }
    $vsPath = (& $vswhere -latest -products * `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath | Select-Object -First 1)
    if (-not $vsPath) { return }
    $vsPath = $vsPath.Trim()
    $devShell = Join-Path $vsPath "Common7\Tools\Microsoft.VisualStudio.DevShell.dll"
    if (-not (Test-Path $devShell)) { return }
    Import-Module $devShell
    Enter-VsDevShell -VsInstallPath $vsPath -SkipAutomaticLocation `
        -DevCmdArguments "-arch=x64 -host_arch=x64" | Out-Null
}

Enable-MsvcEnvironment

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$includeDir = Join-Path $repoRoot "include"
$targetDir = Join-Path $repoRoot "target\$Profile"
$source = Join-Path $PSScriptRoot "main.c"
$exe = Join-Path $PSScriptRoot "c-smoke.exe"

if (-not (Test-Path (Join-Path $includeDir "onebox.h"))) {
    throw "未找到 include\onebox.h，请先在仓库根执行 cargo build"
}

# 优先使用核 cdylib 的导入库，避免手动补齐 Windows 系统链接库。
# 必须明确锁定 onebox.dll.lib：target 目录里还有 onebox_terminal.dll.lib，
# 用 *.dll.lib 通配会选到错误的库。
$importLib = Join-Path $targetDir "onebox.dll.lib"
if (-not (Test-Path $importLib)) { $importLib = $null }
$staticLib = Join-Path $targetDir "onebox.lib"

if (Get-Command cl.exe -ErrorAction SilentlyContinue) {
    if ($importLib) {
        cl.exe /nologo /W3 /utf-8 /I "$includeDir" "$source" `
            /Fo"$PSScriptRoot\main.obj" /Fe:"$exe" `
            /link /LIBPATH:"$targetDir" "$importLib"
        Copy-Item (Join-Path $targetDir "onebox.dll") $PSScriptRoot -Force
    }
    elseif (Test-Path $staticLib) {
        # 静态库方式：需要显式补上 Rust 静态库依赖的 Windows 系统库。
        cl.exe /nologo /W3 /utf-8 /I "$includeDir" "$source" `
            /Fo"$PSScriptRoot\main.obj" /Fe:"$exe" `
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
