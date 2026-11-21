# 升级记录：skyward v0.2.0 → 官方 v0.3.1

执行日期：2026-11-14 · 目标：把工作区源码基线升到官方 `v0.3.1`，**dijun 修复合并留到下一轮**。
本轮没有连设备、没有刷机、没有写任何设备分区；源码已替换为**干净官方基线**，未重打任何 violin 或 dijun 补丁。

## 1. 版本与基线

| | 升级前 | 升级后 |
|---|---|---|
| 源码 | 官方 `v0.2.0` (`22528ba`) + violin 补丁 | 官方 **`v0.3.1`** (`beef595fac2efeb9879ac95de0f2c1baa077ee65`) |
| workspace version / edition | `0.2.0` / `2021` / resolver 2 | **`0.3.1` / `2024` / resolver 3** |
| rust-toolchain | `stable` | **`1.100.0`** + clippy + `aarch64-linux-android` |
| 受控文件数 | 约 230 | **14,138**（大量 `skyward-programs/tests/described/` 测试 fixture） |
| 本地 git | `faf5642` | **`d1ea3f7`** `baseline: upgrade to official skyward v0.3.1` |

**回退保险**（升级前创建，指向已验证的 60/90Hz 构建状态）：
- 分支 `violin-v0.2.0-stable`
- 附注标签 `violin-v0.2.0-verified`

源码包：`.references/skyward-v0.3.1-source.zip`（43,666,286 字节）、解压副本 `.references/v031-src/`。
差异数据：`.references/compare-v0.2.0-v0.3.1.json`（GitHub compare API 只返回前 300 个文件，**已分页截断**，所以升级以 zip 全量文件为准，不以 compare 为准）。

## 2. 官方 0.3.1 的主要变化

- **23 个提交 / 300 个变化文件 / +8528 −627**（compare 可见部分），实际新增 261 个文件。
- 新增 crate：`skyward-omapi`（OMAPI/安全元件，约 1250 行）、`skyward-programs`（从 OTA 描述服务程序，约 1700 行）；
  `skyward-binder` 新增 `serve.rs`/`wire.rs`，`skyward-crypto` 新增 `encryption.rs`，`skyward-sys` 新增 `worker.rs`。
- 打包器新增 `skyward-pack described` 子命令 + `described.rs`：从 OTA 包只下载含 init 脚本/服务描述/账号/build.prop 的分片并校验摘要。
- **`crates/skyward-ui/src/app.rs` 被拆成 `app/` 目录 20 个模块**（`reboot.rs`、`power.rs`、`input.rs` …）。
- `skyward-install/src/run.rs` 改用 `skyward_sys::worker::Worker` 抽象重写安装线程。
- `skyward-block/src/mount.rs` 新增 `Space` / `space()` / `statvfs()`。
- `Service` 配置**删除** `description` 字段，新增 `when`、`ready_hardware_service`。

## 3. 本轮做了什么

1. `git branch violin-v0.2.0-stable` + `git tag -a violin-v0.2.0-verified` 固化旧状态。
2. 下载并解压官方 v0.3.1 源码到 `.references/v031-src/`（14,163 文件）。
3. 按官方文件清单同步工作区：**新增 13,939 / 覆盖 83 / 删除 1**。唯一删除的
   `crates/skyward-ui/src/app.rs` 经核实是被官方拆成 `app/` 目录，属正确动作。
4. **保留的项目产物**（官方 zip 里没有或刻意不覆盖）：
   `devices/violin.toml`、`docs/`、`logs/`、`out/`、`tools/lz4.exe`、`.references/`、`.workbuddy/`。
   `.gitignore` 用官方版本覆盖后追加本地规则 `.references/ out/ tools/ logs/`，
   避免把 42 MB zip 和 100 MiB 镜像提交进仓库。
5. 提交为 `d1ea3f7`，`git status` 干净。

## 4. 编译验证结果（两处失败，都不是源码升级错误）

### 4.1 工具链：`rust-toolchain.toml` 钉死 `1.100.0`

本机默认 `rustc/cargo 1.99.1`。首次 `cargo build` 触发 rustup 自动安装 1.100.0，
下载过程中残留目录导致回滚失败（`os error 145 目录不是空的`），
报 `the 'cargo.exe' binary ... is not applicable to the '1.100.0-x86_64-pc-windows-msvc' toolchain`。

- **已修复**：`rustup toolchain uninstall 1.100.0-x86_64-pc-windows-msvc` 清掉残留，
  再 `rustup toolchain install 1.100.0-x86_64-pc-windows-msvc --component clippy --target aarch64-linux-android`，
  成功装上 `rustc 1.100.0 (0ef1bc386 2026-11-09)`。
- 备用覆盖方式（如以后再遇到工具链问题）：
  `RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc RUSTUP_AUTO_INSTALL=0 cargo ...`
  （edition 2024 从 Rust 1.85 起稳定，本机 1.99.1 够用）。

### 4.2 Windows 宿主：官方只支持 Unix（预期，需下一轮重打适配补丁）

用 stable 覆盖编译 `skyward-pack`，`BUILD_RC=101`，错误集中在两个 crate：

| crate | 错误 |
|---|---|
| `skyward-bootimg/src/ramdisk.rs` | 15 个：`std::os::unix::*`、`symlink`、`lchown`、`mknod`、`utimensat`、`from_mode` 等 |
| `skyward-block/src/mount.rs` | 7 个：`CString`/`OsStrExt::as_bytes`、`libc::statvfs`、`sysconf`、`_SC_PAGESIZE` 等（0.3.1 新加的 `space()`） |

我们 0.2.0 时给 `ramdisk.rs` 打过 `#[cfg(unix)]` 隔离补丁；**0.3.1 又把 `mount.rs` 也变成 unix-only**，
所以下一轮除了重打旧补丁，还要给 `mount.rs` 新增同样的隔离。

工具链修好后用官方 `1.100.0` 复验（`RUSTUP_AUTO_INSTALL=0 cargo build --release -p skyward-pack`）：
`rustc 1.100.0 (0ef1bc386)` 已生效，`BUILD_RC=101`，仍是 `skyward-bootimg/src/ramdisk.rs`
的 15 个 unix-only 错误；用 stable 那次还同时暴露 `skyward-block/src/mount.rs` 的 7 个同类错误。
**结论：工具链问题已排除，剩下的纯粹是官方只支持 Unix 宿主**，两个 crate 的适配都要做。

⚠️ `target/release/skyward-pack.exe` 当前仍是 **0.2.0 的旧产物**（2026-11-13 18:11），
**不要用它打包 0.3.1 镜像**。

## 5. 关键兼容性警告：`violin.toml` 现在能解析，但三个字段被静默忽略

0.3.1 的 config 结构没有 `deny_unknown_fields`，所以旧配置不会报错，但：

| 配置字段 | 0.3.1 结构 | 后果 |
|---|---|---|
| `[display] refresh = 60` | `Display` 只有 `width,height,density,diagonal,corner_radius` | **被忽略** → `surface.rs` 用 `max_by_key(vrefresh)` 选 144Hz → 花屏 |
| `[touch] reversed = ["y"]` | `Touch` 只有 `writes` | **被忽略** → 触摸坐标映射失效 |
| `[[service]] optional = true` | `Service` 无 `optional` | **被忽略** → 服务未就绪会终止 recovery |
| `[modules] load` / `[partitions] mounted` / `[logd] libraries` / `[stock]` | 均保留 | 仍有效 |

**结论：当前工作区不能直接构建出可用的 violin 镜像。** 下一轮必须先把这些字段与新补丁一起重打。

## 6. 下一轮「合并 dijun 修复」的重打清单

按顺序建议一起做（都在新基线上，避免多次刷机）：

### A. 构建链（先做，否则无法编译）
1. `skyward-bootimg/src/ramdisk.rs`：`#[cfg(unix)]` 隔离（0.2.0 补丁重打，注意行号已变：487→456）。
2. `skyward-block/src/mount.rs`：**新增** `space()/statvfs()` 的 `#[cfg(unix)]` 隔离 + `cfg(not(unix))` 桩。
3. `skyward-config/src/lib.rs`：6 处 `Path::is_absolute()` → `starts_with('/')`（Windows 打包前置条件）。

### B. dijun 的 P0 修复（见 `docs/REVIEW-dijun-skyward-2026-11-14.md`）
4. **不引入** PR #30 的每秒 `touch.writes` 重写（dijun 同平台也没有）→ 直接解决长按断触。
5. `surface.rs::turn_off()`：关 CRTC 前完整解绑 plane `FB_ID`/`CRTC_ID`、connector `CRTC_ID`、清 `MODE_ID`；`present()` 成对恢复（`lp.is_none()` 判断非高通）。替换我们的 `Resting` 降级。
6. 打包器：`system/etc/init/logd.rc` 的 `priority 10` → `priority -2` + `rlimit nice 22 22`。
7. `fstab.rs::preserve_o1_data()`：去 `check|formattable|resize`、persist/nvdata `ro,noload`、移除 rescue→/cache。
8. 可选：双击亮屏 `touch.rs` + `KEY_WAKEUP`（NT36536 是否支持 mode 14 需先只读探测）。

### C. 既有 violin 补丁重打（0.3.1 结构已变，不能原样套）
9. `config`：加回 `Display.refresh`、`Touch.reversed`、`Service.optional` 三个字段。
10. `surface.rs`：`KmsSurface::open(path, refresh)` + 60/90Hz 模式选择（0.3.1 仍是 `max_by_key`，954→918 行）。
11. `input.rs`：`swapped` + `reversed=["y"]` 坐标映射（422→333 行，结构基本没变）。
12. `ui.rs`：`normal_max()` 非高通回退；`dual_boot = false`；**0.3.1 的 UI 已拆成 `app/` 目录**，`KmsSurface::open`/`Inputs::open` 调用点需重新定位（2656→3019 行）。
13. `fstab.rs`：动态扫描 `fstab.*`（202→169 行）。
14. `services.rs`：`optional` 容错（357→544 行，改动较大，需重新定位）。
15. `main.rs`：sharedsecret 失败不终止。
16. `install/run.rs` + `systems/install.rs`：双系统封禁（0.3.1 用 `Worker` 重写了安装线程，需重新定位）。
17. `pack/main.rs`：`--system-init` 可选（358→379 行）。
18. **EDL 移除**：0.3.1 在 `crates/skyward-ui/src/app/reboot.rs:13,17` + `pages.rs:154` 仍有 EDL，需按新位置重打。
19. `devices/violin.toml`：`refresh` 固化 **90**（已真机验证），并按 0.3.1 config 结构核对字段。

### D. 不计入本轮
WLAN（用户决定以后再修）、蓝牙、振动（violin 无马达）、旋转、KernelSU 发布方式。

## 7. 构建命令模板（工具链修好后）

```sh
cd C:/Users/shc/Desktop/skyward-violin
# 若 1.100.0 仍未修好，加 RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc RUSTUP_AUTO_INSTALL=0
cargo build --release -p skyward-pack
NDK=C:/Users/shc/AppData/Local/Android/Sdk/ndk/29.0.14206865
BIN=$NDK/toolchains/llvm/prebuilt/windows-x86_64/bin
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$BIN/aarch64-linux-android30-clang.cmd"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_AR="$BIN/llvm-ar.exe"
export CC_aarch64_linux_android="$BIN/aarch64-linux-android30-clang.cmd"
export AR_aarch64_linux_android="$BIN/llvm-ar.exe"
cargo build --release --target aarch64-linux-android -p skyward
```

（`cargo ndk` 在这台机器上曾挂起 16 分钟，沿用直接指定 NDK clang 的方式。）

## 8. 状态一句话

**源码升级已完成并固化**（`d1ea3f7`，官方 v0.3.1 `beef595`），旧状态在分支/标签里可随时回退；
当前是**干净官方基线**，不能构建 violin 镜像，等下一轮按第 6 节清单合并 dijun 修复 + 重打 violin 补丁。
