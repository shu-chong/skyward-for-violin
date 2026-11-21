# 安装卡片字号/配色、用时计时器、data 未解密文案

日期：2026-11-19 20:53
镜像：`out/skyward-violin-OS4.0.5-20261119-2053.img`
大小：104,857,600 B（ramdisk 49,796,386 B，1049 条）
SHA-256：`9991f1522d87d1ed9e0c78403632496e5cce428d877aa609f17b5188ecb5f743`
内嵌 `system/bin/recovery` SHA-256：`9165dfa5ec2c42b4933c1ab2898730f44ee856114a4334901748f15eee18bf82`
（与本机 `target/aarch64-linux-android/release/skyward` 一致）

**未刷机、未写设备分区。**

## 一、安装卡片里包名一行：更大、蓝色

`crates/skyward-ui/src/widgets.rs::progress_card_named`

- 字号：`m.row_title_px`（16.9dp，与标题同大）→ `m.row_title_px + m.dp(4.2)`（21.1dp，约 +25%）。
  逐字换行的宽度测量、行高（`name_px * 1.18`）、首行落点（`title_base + 24dp - ink_top`）都随新字号重算，
  长版本仍按字符硬断行，不会被省略号吃掉两端。
- 颜色：`t.pal.text`（黑/白）→ `t.pal.accent`（浅色 74,128,247 / 深色 84,136,248），与进度条同一支蓝。
- 卡片高度随行高自动增长（单行多约 3.5dp），下方说明、进度条、卡片底边都由同一套变量推导，无需另调。

## 二、「正在安装」右侧增加用时计时器

格式：`用时：00:00`（繁体 `用時：`，英文 `Elapsed 00:00`），分钟两位 + 秒两位，超过 99 分钟照常进位（`120:05`）。

实现分三层：

1. `crates/skyward-ui/src/app/state.rs`
   - 新增 `install_at: Option<u64>`（这台机器时钟的毫秒，安装开始时记下）与
     `install_secs: Option<u32>`（已跑的整秒，供页面直接画）。
2. `crates/skyward-ui/src/app/mod.rs`
   - `tick()`：`Install::Updating` 期间首次 tick 记 `install_at = now`、`install_secs = 0`；
     之后每秒算出整秒、变了才 `refresh()`；离开 `Updating` 立即清零（下一次安装重新计时）。
     **按安装状态计时，不按页面**，所以切页离开再回来不会重置（安装期间导航本来也锁着）。
   - `deadline()`：新增 `ticking`，在 `install_at + (已跑秒数 + 1) * 1000` 触发下一次 tick，
     并 `min(now + 1000)` 防止落后时补帧风暴；进度条卡住不动时数字照样走。
3. `crates/skyward-ui/src/pages.rs` + `widgets.rs`
   - `pages::install` 的 `Install::Updating` 分支把秒数格式化成 `用时：mm:ss` 传给卡片；
   - `progress_card_named(..., elapsed: Option<&str>)`：标题按 `width - 计时器宽 - 10dp` 截断
     （下限 48dp，窄屏不会把标题压没），计时器右对齐到文字区右端、与标题同一基线，字号 `m.sub_px`、色 `t.pal.subtitle`。
   - 计时覆盖整个作业：写包 → 校验 → 写回 Skyward → 合并（`Stage::Merging`），到安装结束为止。

## 三、data 状态：刚进 Skyward 时显示「未解密」

`crates/skyward-ui/src/fmt.rs::data`（首页「解密」行、设置页 data 行、文件页共用同一处文案）

| 状态 | 旧 | 新 |
|---|---|---|
| `Closed`（还没解） | 未打开 / 未開啟 / Not opened | **未解密 / 未解密 / Not decrypted** |
| `Opening`（正在解） | 未打开 / 未開啟 / Not opened | **正在解密 / 正在解密 / Decrypting** |
| `Failed(_)`（解不开） | 未打开 / 未開啟 / Not opened | 未打开 / 未開啟 / Not opened（不变） |
| `Locked { .. }`（等锁屏密码） | 未解密 / 未解密 / Locked | 不变 |
| `Open` / `Erased` | 已解密 / 已清除 | 不变 |

设置页「总容量 · data 分区未打开」那句（`pages.rs` 里存储容量行）说的不是解密状态，未动。

## 四、构建与自检

```sh
NDK=C:/Users/shc/AppData/Local/Android/Sdk/ndk/29.0.14206865
BIN=$NDK/toolchains/llvm/prebuilt/windows-x86_64/bin
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$BIN/aarch64-linux-android30-clang.cmd
export CARGO_TARGET_AARCH64_LINUX_ANDROID_AR=$BIN/llvm-ar.exe
export CC_aarch64_linux_android=$BIN/aarch64-linux-android30-clang.cmd
export AR_aarch64_linux_android=$BIN/llvm-ar.exe

cargo build --release --target aarch64-linux-android -p skyward
PATH=$PWD/tools:$PATH target/release/skyward-pack.exe recovery \
  --stock .references/stock405-recovery.img \
  --binary target/aarch64-linux-android/release/skyward \
  --system-init target/system-init/aarch64-linux-android/release/system-init \
  --config devices/violin.toml \
  --font resources/fonts/MiSansVF.ttf \
  --mono resources/fonts/KirkMono-Regular.ttf \
  --mono-license resources/fonts/KirkMono-OFL.txt \
  --out out/skyward-violin-OS4.0.5-20261119-2053.img
```

| 检查 | 结果 |
|---|---|
| Android release 构建 | RC=0 |
| `cargo clippy -p skyward-ui -p skyward`（android target） | 无新增：skyward-ui 6 条、skyward 3 条均为本轮之前既有 |
| 新文案入二进制 | `用时：` / `用時：` / `Elapsed ` / `正在解密` 均命中 |
| 镜像内 `system/bin/recovery` | 与本机构建产物同哈希 |
| ramdisk 条目 | 1049 条（含 `skyward/system-init`） |
| `repack` 往返 | RC=0，ramdisk 49,796,386 B |
| 配置 | 未改动：`mtp=false`、`mtp_after_decrypt=true`、90Hz、rotate=270、columns=560 |

## 五、这一轮没有做视觉预览

`skyward-preview` 依赖 `skyward-info`，后者用 `std::os::unix` 和 `rsproperties`，**Windows 宿主编不出来**
（`cargo build -p skyward-preview` 在 `rsproperties` 处报 26 个错），所以本轮无法像之前那样先把页面渲染成 PNG 自查。
版面按 dp 手算复核：卡片文字区在 violin 上约 496dp，标题 4 字 ≈ 68dp + 计时器 ≈ 95dp，不会相撞；
包名 21.1dp 下 28 字符的 `17OS4.0.261112.190429467.XPRDCN.S` 折成 2 行，行高 24.9dp > 字面高度，不会叠字。
仍需真机确认。

`crates/skyward-preview/src/main.rs` 顺带把 `33-sideload-writing-dark` 的版本换成上面这个长名，
并给 `Install::Updating` 的预览帧塞 `install_secs = 312`（05:12），方便以后在能编译的宿主机上看新卡片。

## 六、回退

`out/skyward-violin-OS4.0.5-20261119-0935.img`（不含装完即合并）。
上一版 `…2003.img`（含合并、不含本轮 UI 改动）已删除。
