# Skyward — violin 适配版

本目录是 [kirklin/skyward](https://github.com/kirklin/skyward) **官方 v0.3.1（commit `beef595`）**
在 **Xiaomi Pad 7S Pro 12.5（codename `violin`，XRING O1）** 上的适配版。
源码已整理成可直接构建、可直接推送的状态；从官方源码复现本目录的改动见 [`patches/`](patches/README.md)。

上游许可证见 [`LICENSE`](LICENSE)（GPL-3.0），上游原始说明见 [`README.md`](README.md) /
[`README.zh-CN.md`](README.zh-CN.md)。本文件只记录 violin 这一支的差异。

---

## 1. 设备与配置

目标机：Xiaomi Pad 7S Pro 12.5（25053RP5CC），XRING O1，HyperOS 4.0.x。
当前底座是 **OS 4.0.6** 的 recovery，内核 `6.18.21-android17-…` 来自 boot 分区，
recovery 分区只带 ramdisk，不需要随 OTA 换内核。

`devices/violin.toml` 的关键取值（改动前请先看 `docs/` 里的原因）：

| 项 | 值 | 原因 |
| --- | --- | --- |
| `mtp` | `false` | 开机时 MTP gadget 重绑会让 Windows 丢掉 adb；数据解密后再开 |
| `mtp_after_decrypt` | `true` | 解密后自动恢复 MTP |
| `rotate` | `270` | 面板横向扫描，UI 按竖向排版后旋转 90° |
| `columns` | `560` | 平板双栏布局；单栏会让分栏圆角动画失效 |
| `refresh` | `120` | **已验证上限**。144 会持续 DPU underflow，原因见 `docs/REVIEW-144hz-failure-2026-11-19.md` |
| AVB 验证按钮 | 仍禁用 | 不要恢复 |

刷新率的配套改动：`crates/skyward-sys/src/dvfs.rs` 在面板点亮前把
`/sys/class/devfreq/ddr_devfreq/min_freq` 钉到最高档（仅 `refresh > 90` 时）。
这是 120 Hz 能稳定的关键；90 Hz 不需要。

## 2. 构建

工具链：Rust 1.100.0、cargo-ndk 4.1.2、Android NDK 29.0.14206865。
仓库自带 `rust-toolchain.toml`。`skyward-pack` 调用外部 `lz4` CLI，见 [`tools/README.md`](tools/README.md)。

```sh
NDK=C:/Users/shc/AppData/Local/Android/Sdk/ndk/29.0.14206865
BIN=$NDK/toolchains/llvm/prebuilt/windows-x86_64/bin
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$BIN/aarch64-linux-android30-clang.cmd
export CARGO_TARGET_AARCH64_LINUX_ANDROID_AR=$BIN/llvm-ar.exe
export CC_aarch64_linux_android=$BIN/aarch64-linux-android30-clang.cmd
export AR_aarch64_linux_android=$BIN/llvm-ar.exe

# Windows 上 cargo build --workspace 会卡死，按包逐个构建
cargo build --release --target aarch64-linux-android -p skyward
cargo build --release -p skyward-pack
RUSTFLAGS="-C target-feature=+crt-static" cargo build --release \
  --target aarch64-linux-android -p skyward-system-init --target-dir target/system-init

PATH=$PWD/tools:$PATH target/release/skyward-pack.exe recovery \
  --stock <4.0.6 的 recovery.img> \
  --binary target/aarch64-linux-android/release/skyward \
  --system-init target/system-init/aarch64-linux-android/release/system-init \
  --config devices/violin.toml \
  --font resources/fonts/MiSansVF.ttf \
  --mono resources/fonts/KirkMono-Regular.ttf \
  --mono-license resources/fonts/KirkMono-OFL.txt \
  --out out/skyward-0.3.1-violin-OS4.0.6-<YYYYMMDD-HHMM>.img
```

`--out` 可以不给：打包器自动按
`out/skyward-<版本>-<codename>-<底座版本>-<时间>.img` 命名，底座版本取自
`prop.default` 的 `ro.build.version.incremental`。时区依赖 `jiff` 的 `tz-system` feature，
Windows 上不要去掉。

## 3. 本支相对官方改了什么

| 范围 | 文件数 | 内容 |
| --- | --- | --- |
| UI / 排版 | ~20 | 竖向画布 + 双栏、状态栏/充电层、安装卡片重排、绿色路径、版本号放大、翻页圆角修复、解密倒计时 |
| 安装 / OTA | ~10 | 装完即合并、真实路径与包名显示、`post-build-incremental` 来源、保护/校验分支 |
| 槽位 / 快照 | 4 | `skyward-vab` 的合并流程与状态显示 |
| 显示 / 休眠 | 6 | 120 Hz、DDR 保底、MTP 解密后再开、背光/窗帘层 |
| 打包 | 2 | 自动命名、字体与配置打包、`--out` 可选 |
| 设备配置 | 1 | `devices/violin.toml`（新增） |
| 其他 | ~20 | 上游修复回合、日志与诊断辅助 |

完整逐文件清单在 `patches/0001-violin-port.patch`；具体原因按主题写在 `docs/`。

## 4. 已验证 / 已知限制

**已验证**：装完即合并后重启不再卡 Mi（`/metadata/ota/state` = `08 05`）；
120 Hz 连续运行 + 滑动翻页 + 息屏唤醒，`underflow` 0 次；
4.0.6 底座与 4.0.5 的 ramdisk 只有版本身份字符串不同，内核版本一致。

**已知限制**：

- 144 Hz 在本机无解：DDR 已钉到最高频仍持续下溢，DPU 核心只有 483.33 MHz
  （`clk profile 1`，recovery 无 vendor perf 投票），120 Hz 需要 1.89 px/clk，
  144 Hz 需要 2.14 px/clk。要上 144 得改内核/驱动或开 DSC。
- 120 Hz 已占 DPU 约 94.4% 能力，余量很小。
- Windows 上 `skyward-preview` 编不过（依赖 `skyward-info` 的 unix-only API）；
  宿主单测受 `skyward-lp` 的 `read_exact_at` 影响，同样只适合在 Linux 跑。
- `skyward-ui` / `skyward` 有少量历史 clippy 告警，非本支引入，未处理。
- 60/90 Hz 与 120 Hz 用同一套逻辑；90 Hz 回退镜像仍保留在旧工作区，不在本目录。

## 5. 目录说明

```
.github/ assets/ resources/ slides/   上游自带，未改动
crates/                              适配后的 Rust 源码
devices/violin.toml                  violin 设备配置（本支新增）
docs/                                移植过程与验证记录
patches/                             从官方 v0.3.1 复现本支的 patch
tools/README.md                      外部 lz4 工具的获取方式
```

**未包含**：`.references/`（官方源码包、OTA/recovery 镜像）、`out/`（构建产物）、
`target/`、设备日志、截图、`.workbuddy/`（本机工作记录）、`tools/lz4.exe`（第三方二进制）。
这些都与构建或发布无关，或不应进入公开仓库。

## 6. 推送

本目录是一个全新的 git 仓库，只有一个干净的初始提交、**没有配置 remote**：

```sh
cd skyward-violin-github
git remote add origin <你的 fork 地址>
git push -u origin master
```

如果更想保留此前逐次提交的历史，请从原工作区推送；本目录刻意不复制原先的
`.git`，因为旧历史里含有本机工作记录（`.workbuddy/memory/`）。
