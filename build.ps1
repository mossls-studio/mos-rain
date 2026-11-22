# 构建并安装 mos_雨滴（EffectCraft 外部效果辅助程序 v5.0.0）
# 用法：在目录内执行  .\build.ps1

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$appData = $env:APPDATA
if (-not $appData) { $appData = [Environment]::GetFolderPath('ApplicationData') }

Write-Host '[1/5] 激活 Rust 工具链 ...'
if (Test-Path 'D:\ruanjian\rust-env\activate.ps1') {
    . 'D:\ruanjian\rust-env\activate.ps1'
    $env:CARGO_NET_OFFLINE = 'true'
} else {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if (-not $cargo) { throw '找不到 cargo，也未发现 D:\ruanjian\rust-env\activate.ps1' }
    Write-Host "    使用系统 $($cargo.Source)"
}

Write-Host '[2/5] 编译 helper ...'
Push-Location (Join-Path $here 'helper')
# MinGW 的 ld 在含非 ASCII 字符路径下可能找不到 deps 里的对象文件，所以把 target 目录放在 TEMP 下。
$targetDir = Join-Path $env:TEMP 'mos-rain-target'
try {
    & cargo build --release --target-dir $targetDir
    if ($LASTEXITCODE -ne 0) { throw "编译失败（cargo 退出码 $LASTEXITCODE），没有安装任何文件" }
} finally {
    Pop-Location
}

$exe = Join-Path $targetDir 'release\effectcraft-helper-mos-rain.exe'
if (-not (Test-Path $exe)) { throw "编译失败：找不到 $exe" }

Write-Host '[3/5] 安装 helper 到 dist 和 EffectCraft ...'
Get-Process effectcraft-helper-mos-rain -ErrorAction SilentlyContinue | ForEach-Object {
    $_.Kill()
    $_.WaitForExit()
}

$distDir = Join-Path $here 'dist'
New-Item -ItemType Directory -Force -Path $distDir | Out-Null
Copy-Item $exe (Join-Path $distDir 'effectcraft-helper-mos-rain.exe') -Force

$helpersDir = Join-Path $appData 'EffectCraft\helpers'
New-Item -ItemType Directory -Force -Path $helpersDir | Out-Null
Copy-Item $exe (Join-Path $helpersDir 'effectcraft-helper-mos-rain.exe') -Force

Write-Host '[4/5] 停用旧版纯 wasm 版（避免 mos.rain ID 冲突）...'
$oldWasm = Join-Path $appData 'EffectCraft\Plug-ins\mos_rain.wasm'
if (Test-Path $oldWasm) {
    $disabledDir = Join-Path $appData 'EffectCraft\Plug-ins-disabled'
    New-Item -ItemType Directory -Force -Path $disabledDir | Out-Null
    Move-Item $oldWasm (Join-Path $disabledDir 'mos_rain.wasm') -Force
    Write-Host "    已将旧版移至 $disabledDir\mos_rain.wasm"
} else {
    Write-Host '    未发现旧版 mos_rain.wasm，跳过'
}

Write-Host '[5/5] 确保开机自加载脚本存在 ...'
$startup = Join-Path $appData 'EffectCraft\Scripts\Startup'
New-Item -ItemType Directory -Force -Path $startup | Out-Null
Copy-Item (Join-Path $here 'tools\mos-load-helpers.jsx') (Join-Path $startup 'mos-load-helpers.jsx') -Force

Write-Host ''
Write-Host "完成：$helpersDir\effectcraft-helper-mos-rain.exe"
Write-Host '重启 EffectCraft 后，效果列表「MOS 形变」分类下会自动加载 mos_雨滴 v5.0。'
Write-Host '（当前已开着的窗口：执行一次 效果 ▸ 扫描效果辅助程序... 即可立刻加载）'
