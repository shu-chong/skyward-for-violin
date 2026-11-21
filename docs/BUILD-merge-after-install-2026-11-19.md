# 装完即合并版：构建与交付

日期：2026-11-19 20:03
镜像：`out/skyward-violin-OS4.0.5-20261119-2003.img`
大小：104,857,600 B（ramdisk 49,795,329 B）
SHA-256：`6cf2580f3c4fd25f66ff552e0b38a9f6bc5256ec8cb7102fe4a6ca38a38482b3`
内嵌 `system/bin/recovery` SHA-256：`466749556832f91afb7fdcc127913aa96b66ad6b00fda0c6c370da4cf7e4838a`
（与本机 `target/aarch64-linux-android/release/skyward` 一致）
实现说明：`docs/FEATURE-merge-after-install-2026-11-19.md`

**未刷机、未写设备分区。**

## 构建步骤

```sh
NDK=C:/Users/shc/AppData/Local/Android/Sdk/ndk/29.0.14206865
BIN=$NDK/toolchains/llvm/prebuilt/windows-x86_64/bin
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$BIN/aarch64-linux-android30-clang.cmd
export CARGO_TARGET_AARCH64_LINUX_ANDROID_AR=$BIN/llvm-ar.exe
export CC_aarch64_linux_android=$BIN/aarch64-linux-android30-clang.cmd
export AR_aarch64_linux_android=$BIN/llvm-ar.exe

cargo build --release --target aarch64-linux-android -p skyward            # RC=0
cargo build --release -p skyward-pack                                     # RC=0

# system-init（0.2.0 起打包器会把它放进 ramdisk 的 skyward/system-init）
RUSTFLAGS="-C target-feature=+crt-static" cargo build --release \
  --target aarch64-linux-android -p skyward-system-init --target-dir target/system-init

PATH=$PWD/tools:$PATH target/release/skyward-pack.exe recovery \
  --stock .references/stock405-recovery.img \
  --binary target/aarch64-linux-android/release/skyward \
  --system-init target/system-init/aarch64-linux-android/release/system-init \
  --config devices/violin.toml \
  --font resources/fonts/MiSansVF.ttf \
  --mono resources/fonts/KirkMono-Regular.ttf \
  --mono-license resources/fonts/KirkMono-OFL.txt \
  --out out/skyward-violin-OS4.0.5-$(date +%Y%m%d-%H%M).img
```

## 自检

| 检查 | 结果 |
|---|---|
| Android release 构建 | RC=0，无新增 warning |
| `skyward-pack` 宿主构建 | RC=0，仅 `skyward-bootimg` 既有 4 个 cfg 桩 warning |
| `repack` 往返 | RC=0，ramdisk 49,795,329 B（与源一致） |
| ramdisk 条目 | 1049 条：比 0935 多**恰好一条** `skyward/system-init`（见下） |
| 镜像内 `system/bin/recovery` 哈希 | 与本机构建产物相同 |
| 新标记（二进制内） | `snapshots of slot` / `merged into slot` / `was cut off; it goes on` / `正在合并更新` / `安装成功，待合并更新` 全部命中 |
| 配置 | 未改动：`mtp=false`、`mtp_after_decrypt=true`、90Hz、rotate=270、columns=560 |
| clippy（本轮改动 crate） | 无告警 |

## 和 0935 的一处差异（重要）

`out/skyward-violin-OS4.0.5-20261119-0935.img` 的 ramdisk **没有** `skyward/system-init`：
那次构建没有传 `--system-init`（打包器里它是可选参数）。缺了它，`双系统`
（安装第二个系统）会在运行时找不到 `/skyward/system-init` 而失败。
本轮按 v0.3.1 的完整参数重新传入，ramdisk 只多这一条。

## 回退

`out/skyward-violin-OS4.0.5-20261119-0935.img`（本轮之前，功能上少"装完即合并"）。
