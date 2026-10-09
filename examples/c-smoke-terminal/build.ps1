# 编译并链接终端模块的 C 冒烟测试。
#
# 前置：先在仓库根执行 `cargo build`（或 `cargo build --release`）。
# 该命令会生成 target/<profile>/onebox_terminal.dll 与 include/onebox_terminal.h。
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
$exe = Join-Path $PSScriptRoot "c-smoke-terminal.exe"

if (-not (Test-Path (Join-Path $includeDir "onebox_terminal.h"))) {
    throw "未找到 include\onebox_terminal.h，请先在仓库根执行 cargo build"
}

# 优先使用终端 cdylib 的导入库，避免手动补齐 Windows 系统链接库。
$importLib = Join-Path $targetDir "onebox_terminal.dll.lib"
if (-not (Test-Path $importLib)) { $importLib = $null }
$staticLib = Join-Path $targetDir "onebox_terminal.lib"

if (Get-Command cl.exe -ErrorAction SilentlyContinue) {
    if ($importLib) {
        cl.exe /nologo /W3 /utf-8 /I "$includeDir" "$source" `
            /Fo"$PSScriptRoot\main.obj" /Fe:"$exe" `
            /link /LIBPATH:"$targetDir" "$importLib"
        Copy-Item (Join-Path $targetDir "onebox_terminal.dll") $PSScriptRoot -Force
    }
    elseif (Test-Path $staticLib) {
        cl.exe /nologo /W3 /utf-8 /I "$includeDir" "$source" `
            /Fo"$PSScriptRoot\main.obj" /Fe:"$exe" `
            /link /LIBPATH:"$targetDir" "$staticLib" `
            kernel32.lib advapi32.lib ws2_32.lib userenv.lib bcrypt.lib ntdll.lib
    }
    else {
        throw "未找到可链接的 onebox_terminal 库（既无 *.dll.lib 也无 onebox_terminal.lib）"
    }
}
elseif (Get-Command gcc -ErrorAction SilentlyContinue) {
    gcc -Wall -I "$includeDir" "$source" -L "$targetDir" -lonebox_terminal -o "$exe"
    Copy-Item (Join-Path $targetDir "onebox_terminal.dll") $PSScriptRoot -Force -ErrorAction SilentlyContinue
}
else {
    throw "未找到 C 编译器（cl.exe 或 gcc）。请安装 VS Build Tools 或 MinGW。"
}

Write-Host "已生成 $exe"
