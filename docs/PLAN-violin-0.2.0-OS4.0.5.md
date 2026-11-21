# violin 适配 Skyward 0.2.0 + 官方 ROM OS4.0.5.0 —— 差异分析与适配方案（待确认）

日期：2026-11-12
状态：**方案阶段**。本轮只做了下载与只读对比，没有修改任何源码、配置、镜像或设备。
旧工作区 `C:\Users\shc\Desktop\skyward` 全程未写入（`git status` 仍是原来的 9 个改动 + `devices/violin.toml` + `out/`）。

---

## 1. 已完成的事

| 项目 | 结果 |
|---|---|
| 新源码目录 | `C:\Users\shc\Desktop\skyward-violin`，取自 `v0.2.0` tag（commit `22528ba`，2026-11-08） |
| 基线提交 | `d6508e8 baseline: skyward v0.2.0 (commit 22528ba) for violin OS4.0.5 port`，225 个受控文件 |
| 下载原件 | `.references/skyward-v0.2.0-source.zip`（16,105,714 字节，SHA-256 前缀 `7ff63f230b19d2b3514f6d70043086c5`） |
| 旧补丁存档 | `.references/violin-patches-v0.1.2.diff`（451 行，9 个文件的完整 diff） |
| 旧配置存档 | `.references/violin-4.0.0.14.toml`（222 行） |
| 设备操作 | 无。没有连 adb，没有刷机，没有挂载/写入任何设备分区 |

> GitHub 直连被代理 502 中断，最终走 `codeload.github.com` 拿到官方 tag 的 zipball；
> 因没有完整 git 历史，新目录用 `git init` 建了本地基线，并保留 `origin` 指向官方仓库。

---

## 2. 版本基线

| | 旧 | 新 |
|---|---|---|
| Skyward | `v0.1.2` `c9deaf0` + 9 个未提交补丁 + `devices/violin.toml` | `v0.2.0` `22528ba`（`main` 最新 `ff1668f`，只多 2 个文档/CI 提交） |
| 版本跨度 | — | 47 个提交、89 个文件变更（`v0.1.2...v0.2.0`） |
| violin ROM | `OS4.0.0.14.XOTCNXM` / `7eca225713` | `OS4.0.5.0.XOTCNXM` / `7f96a43945` |
| stock recovery SHA-256 | `15b359bb…5a9`（与旧配置记录一致） | **`376631cfc1988048a1bb32579d5611c5bc8a66c3246c74e5a98b6c9ac4730293`** |

新哈希口径已核对：`001_DNA/recovery.img` 与 `recovery/recovery.img` 都是同一个 100 MiB 文件，
旧配置里写的正是这个口径，所以 4.0.5 直接换成上面的值即可。

---

## 3. ROM 4.0.5 vs 4.0.0.14：**几乎没变，好消息**

| 检查项 | 结果 | 对配置的影响 |
|---|---|---|
| fstab 文件名与位置 | 仍是 `vendor/etc/fstab.O1_asic`，`first_stage_ramdisk/fstab.O1_asic` | 不需要改；仍需 fstab 多文件扫描补丁 |
| `persist` / `nvdata` 挂载点 | 仍是 `/mnt/vendor/persist`、`/mnt/vendor/nvdata` | `[partitions] mounted` 原样保留 |
| `vendor_dlkm` 的 `.ko` | 194 → 193，只删了 `xr_pmonitor.ko` | 无关 |
| `modules.load` 中触摸/WLAN 模块 | 名字与顺序全部不变（行号整体 -1）：`cfg80211`(1)、`pcie_xring`(5)、`connfem/conninfra/wlan_page_pool/wmt_chrdev_wifi_connac3/wlan_drv_gen4m_eap_6639`(152–156)、`xiaomi_touch_violin`(169) | `[modules].load` / `[wlan].modules` 原样保留 |
| 触摸 | `vendor.xiaomi.hw.touchfeature-service.rc`、`/dev/xiaomi-touch`、`/sys/class/touch/touch_dev/enable_touch_raw` 都在 | `[touch].writes`、`[[service]]` 原样保留 |
| 解密 | `tee-supplicant`、keymint/gatekeeper/boot 服务二进制都在，`system/system/lib64/bootstrap` 存在 | `libraries` 里的 `/mnt/system/...` 补路径仍需 |
| WLAN 服务 | `conninfra_loader`、`wlan_assistant`、`nvram_daemon`、`wpa_supplicant` 都在；`init.wlan_drv.rc` 内容与旧版一致 | 见第 6 节的 WLAN 决策 |
| recovery ramdisk | **文件清单与 `init.recovery.O1_asic.rc` 与 4.0.0.14 逐字节一致（diff 为空）** | 打包基线的行为可预期 |
| 振动 | 仍无马达（只有 audio HAL 的 `vibrator_audio_socket_0`，不是马达） | 仍然不写 `[haptics]`，不需要删任何代码 |

**结论：`devices/violin.toml` 除了 `[stock]` 三项，其余路径、模块名、服务名可以整体复用。**
0.2.0 官方设备配置相对 0.1.2 也只多了 `[haptics] resonance` 一个字段，violin 无马达，不受影响。

需要更新的 `[stock]`：

```toml
[stock]
version = "OS4.0.5.0.XOTCNXM"
package = "https://bkt-sgp-miui-ota-update-alisgp.oss-ap-southeast-1.aliyuncs.com/OS4.0.5.0.XOTCNXM/violin-ota_full-OS4.0.5.0.XOTCNXM-user-17.0-7f96a43945.zip"
recovery = "376631cfc1988048a1bb32579d5611c5bc8a66c3246c74e5a98b6c9ac4730293"
```

`package` 是按旧格式推定的，**没有官方出处**；配置校验只要求 `https://`、`.zip` 结尾且文件名含 version。
本轮打包直接用本地 `001_DNA/recovery.img`，不依赖这个 URL（见第 5 节）。

---

## 4. 源码 0.2.0：**我们之前的 9 个补丁一个都没被上游吸收**

| 旧补丁 | 作用 | 0.2.0 状态 | 重打难度 |
|---|---|---|---|
| `skyward-config/src/lib.rs` | `Display.refresh` / `Touch.reversed` / `Service.optional`；Windows 下 `Path::is_absolute()` 误判 Linux 绝对路径 | 结构体字段顺序与旧版基本一致，`is_absolute()` 反而有 6 处（wipe logs / haptics resonance / logd libraries / owners paths / wlan firmware） | 低，按新行号重打 |
| `skyward-ui/src/surface.rs` | 按配置刷新率选 mode，避开 144Hz DPU underflow | 仍 `max_by_key(vrefresh)`，**且 0.2.0 新增了「preferred 上电 → to_fastest 切换」机制** | **需重新设计，见 4.1** |
| `skyward/src/input.rs` | `swapped` 自动轴对调 + `reversed=["y"]` 镜像 | 仍无坐标映射；多了 `Wakeful`、`slot0_value`，`Panel`/`Inputs` 结构仍对应 | 中，按新代码适配 |
| `skyward/src/ui.rs` | `normal_max()` 读不到 `msm_drm` 时回退到 `max_brightness`；`run()` 传 refresh/reversed | `normal_max` 仍硬读 `/sys/module/msm_drm/parameters/dsi_display0`；`run()` 在 1054/1057 行调用，签名未变 | 低 |
| `skyward-block/src/fstab.rs` | 扫描 `/vendor/etc`、`/odm/etc` 下所有 `fstab.*` | **与 0.1.2 完全相同**（`FILES` 仍是两个 `fstab.qcom`） | 低，可直接套 |
| `skyward/src/services.rs` | `optional` 服务 ready 失败不终止 recovery | `ready()` / `start()` 结构与旧版一致 | 低，可直接套 |
| `skyward/src/main.rs` | `agree_on_the_notes_key` 失败改为记日志 | 函数在 603 行，内容与旧版一致 | 低，可直接套 |
| `README.md` / `README.zh-CN.md` | 各一行说明 | 无关紧要 | 可忽略 |

### 4.1 必须重新设计的一个点：0.2.0 的刷新率时序

0.1.2 是「直接用选中的 `mode` 上电」，所以我们把 `mode` 选成 60Hz 就解决了花屏。
0.2.0 改成了：

- `surface.rs:646-647`：`blob = 选中 mode`，`first = preferred`（面板的 PREFERRED mode），
- `surface.rs:709-712`：用 `first` 提交上电，`fastest = (mode.vrefresh() != preferred.vrefresh()).then_some(blob)`，
- `surface.rs:752`：`open()` 末尾立即 `to_fastest()` 切到 `mode`，
- `surface.rs:893/902` 与 `ui.rs:1476`：熄屏唤醒后 `slow = true`，`speed_up()` 再次 `to_fastest()`。

也就是说，**只把 `mode` 选成 60Hz 不够**：若面板的 PREFERRED 是 144Hz，开机瞬间仍以 144Hz 上电，
亮屏/唤醒时还会先回 144Hz 再切回来，花屏可能复现。适配时需要让配置的刷新率同时决定
「上电用的那个 mode」并把 `fastest` 置空（或让 `preferred` 指向选定的 60Hz mode），
并确认 `speed_up()` 不会把它切走。这一条要在第一版真机 log 里重点验证。

### 4.2 Windows 宿主上不重打绝对路径补丁会直接打包失败

`skyward-pack recovery` 第一步就调用 `DeviceConfig::load(&config)` 做校验
（`crates/skyward-pack/src/main.rs:118`）。0.2.0 的校验里有 6 处 `path.is_absolute()`，
Windows 的 `Path` 不把 `/system/...` 当绝对路径 → 配置校验失败 → **镜像根本打不出来**。
所以 `starts_with('/')` 这组修复不是可选项，是构建前置条件。

---

## 5. 构建链的变化（0.2.0 与旧流程完全不同）

旧流程用的是我们临时写的 `tools/repack_recovery.py` + `tools/check_rebuild.py`；0.2.0 自带官方打包器：

```sh
cargo ndk -t arm64-v8a --platform 30 build --release -p skyward
cargo build --release -p skyward-pack
RUSTFLAGS="-C target-feature=+crt-static" cargo ndk -t arm64-v8a --platform 30 build --release -p skyward-system-init --target-dir target/system-init

target/release/skyward-pack recovery \
  --stock "<4.0.5>/001_DNA/recovery.img" \
  --binary target/aarch64-linux-android/release/skyward \
  --system-init target/system-init/aarch64-linux-android/release/system-init \
  --config devices/violin.toml \
  --font resources/fonts/MiSansVF.ttf \
  --mono resources/fonts/KirkMono-Regular.ttf \
  --mono-license resources/fonts/KirkMono-OFL.txt \
  --out out/skyward-violin-OS4.0.5-$(date +%Y%m%d-%H%M).img
```

要点：

- **多一个静态链接的 `skyward-system-init`**（双系统的第一个程序），这是 0.2.0 新增的必填 `--system-init`。
- 打包器会往 ramdisk 里加 5 个命令软链（`7za`/`avbctl`/`resetprop`/`unzip`/`zstd`）、改写 `prop.default`、
  补 `task_profiles.json` 与 MTP rc，并**重新生成未签名的 AVB footer**。
- `ramdisk.rs:279` 的 `lz4_tool()` 仍调用外部 `lz4` CLI 做 legacy 压缩 → 需要把
  旧目录的 `tools/lz4.exe` 复制到新目录并加入 PATH（只读复制，不改旧目录）。
- 本机工具链已确认可用：cargo `1.99.1`、cargo-ndk `4.1.2`、NDK `29.0.14206865`（r29）。
  Windows 上 `ring` / `cl.exe` 偶发卡死的老问题仍可能复现，届时按旧办法杀进程重试。
- `[stock]` 只用于 `skyward-pack stock` 下载校验；本轮用本地镜像，**不需要下载几个 GB 的 OTA**。

---

## 6. 建议的执行顺序（等你确认后才动手）

1. **重打 9 个源码补丁**，其中 `surface.rs` 按 4.1 重新设计；提交一次 `port: violin fixes on 0.2.0`。
2. **写 `devices/violin.toml`**：以 `.references/violin-4.0.0.14.toml` 为底，只改 `[stock]` 三项；
   其余（60Hz、触摸 `reversed=["y"]`、touchfeature、解密服务、persist/nvdata、WLAN、backup、wipe）原样保留。
3. **本地构建 + 打包**，先跑 `skyward-pack list/cat/repack` 自检 ramdisk 完整性，核对输出镜像 SHA-256。
4. **真机验证**：由你刷入后抓 log，优先确认 ①显示不花屏 ②触摸准确 ③data 解密；WLAN 单独一轮。
5. 全部通过后再考虑 WLAN 时序（见下）。

---

## 7. 需要你拍板的三件事

**A. 源码基线**：现在下的是 `v0.2.0` tag（`22528ba`）。要不要换成 `main` 最新 `ff1668f`？
两者只差 2 个提交（README 图片 + CI 里 cargo-ndk 构建方式），功能代码相同。建议保持 `v0.2.0`。

**B. WLAN 这一轮怎么处理**（上一轮 `REVIEW-violin-wlan-2026-11-07.md` 的结论）：
- **B1（建议）先等价移植**：WLAN 配置原样搬过来，先把「0.2.0 + 4.0.5」做成一条可用的基线；
  之后单独一轮按方案 A 修时序（把整套 wlan 模块移到 `[modules].load`、`rfkill` 排第一、
  services 顺序改为 `conninfra_loader → nvram_daemon → wlan_assistant`，`[wlan].modules = []`，保留写 `"1"`）。
- **B2 一步到位**：这一轮就把方案 A 一起做。好处是少刷一次机，坏处是两个变量叠加，
  出问题不好定位，而且 0.2.0 的刷新率时序本身也是新变量。

**C. `[stock].package` URL**：我没有 4.0.5 的官方下载地址，只有本地解包目录。
是按旧格式把推定 URL 写进配置（仅作记录，不影响本次打包），还是你给一个准确 URL？
另外 0.2.0 新增的**双系统安装**会写 boot 分区，本轮建议只在 recovery 里保留功能、不在真机测试，
继续遵守「不修改系统分区、不损坏设备」。

---

## 8. 已知风险

- 0.2.0 的 `skyward-pack` 第一次在 violin 的 100 MiB recovery 分区 + lz4 legacy ramdisk 上跑，
  可能出现 ramdisk 编码/AVB footer 兼容问题；打包后必须先用 `list`/`cat`/`repack` 自检再交付。
- 刷新率逻辑重构后，若 `preferred` 与选定 mode 不同，熄屏唤醒路径仍需真机 log 验证。
- 4.0.5 的 `prop.default`、`sepolicy`、`plat_*_contexts` 与旧版一致（ramdisk diff 为空），
  但 0.2.0 打包器会改写 `prop.default`，需在自检中确认改动内容符合预期。
- 全程只生成 recovery 镜像、由你手动刷入；不连设备写分区、不动 system/vendor。

---

## 附录：证据路径

- 新源码：`C:\Users\shc\Desktop\skyward-violin`
- 旧补丁与配置：`.references/violin-patches-v0.1.2.diff`、`.references/violin-4.0.0.14.toml`
- 4.0.5 ROM：`C:\Users\shc\Desktop\other\DNA_WIN-230828\DNA_violin-ota_full-OS4.0.5.0.XOTCNXM-user-17.0-7f96a43945`
- 4.0.0.14 ROM：`...\DNA_violin-ota_full-OS4.0.0.14.XOTCNXM-user-17.0-7eca225713`
- 上一轮 WLAN 复盘：`C:\Users\shc\Desktop\skyward\docs\REVIEW-violin-wlan-2026-11-07.md`（只读引用）
- 官方构建说明：`README.md` 146–193 行；打包器参数：`crates/skyward-pack/src/main.rs` 45–86 行

---

## 9. 实施记录（2026-11-12，用户已确认）

用户确认：**保持 `v0.2.0` tag；这一轮不做 WLAN，按不支持 WLAN 构建；不做双系统，按不支持双系统构建**；
4.0.5 官方 OTA 地址为
`https://bkt-sgp-miui-ota-update-alisgp.oss-ap-southeast-1.aliyuncs.com/OS4.0.5.0.XOTCNXM/violin-ota_full-OS4.0.5.0.XOTCNXM-user-17.0-7f96a43945.zip`。

### 9.1 不支持 WLAN 的做法

`devices/violin.toml` 里整段 `[wlan]` 删除（`Option<Wlan>` 为 None 时 `ui.rs:1355` 的
`wlan` worker 不启动，`app.rs:1918` 的 Wlan 页面不可达）。连带删除：

- `[modules].load` 中的 `conninfra`；
- `conninfra_loader`、`wlan_assistant` 两个 `[[service]]`；
- `[partitions].mounted` 中的 `nvdata`（只留 `persist`，解密要用）。

于是 `/dev/wmtWifi` 不会被写、connsys 芯片不上电、NVRAM 分区不挂载。
`[backup].efs` 里仍保留 `nvdata` 等分区名，那只是备份清单，与挂载无关。

### 9.2 不支持双系统的做法（四道）

0.2.0 的双系统耦合到 UI 状态、Systems 页面、worker、安装器和打包器，硬删代码风险过高，
因此按「功能不可达 + 镜像里没有启动程序」来构建：

1. `crates/skyward/src/ui.rs`：`app.state.dual_boot` 强制 `false`。上游
   `app.rs:1918` 的 `Screen::Systems => self.dual_boot` 让页面不可达，
   `systems_worker` 为 `None`，fastboot 的 boot-set 回写分支不再进入。
2. `crates/skyward-install/src/run.rs`：分派到 `Package::System(_)` 时直接
   `bail!`，在写任何分区之前拒绝第二系统包；`install_system` 保留但标
   `#[allow(dead_code)]`；`installed_boot_set()` 改为空操作。
3. `crates/skyward-systems/src/install.rs`：`ensure_installed()` 改为恒返回
   `Ok(false)` 的桩。备份、擦除、分区、切槽等流程里那 5 处调用由此全部
   不会写 boot 分区。
4. `crates/skyward-pack/src/main.rs`：`--system-init` 改为可选参数，本构建
   不传，镜像里不写入 `skyward/system-init`。因此也不需要编译
   `skyward-system-init`（省掉静态链接那一步）。

`skyward-systems` 库仍参与编译（UI 类型引用），但不会执行任何双系统工作。

### 9.3 重打的补丁（0.1.2 → 0.2.0）

| 文件 | 改动 |
|---|---|
| `crates/skyward-config/src/lib.rs` | 加 `Display.refresh`、`Touch.reversed`、`Service.optional`；6 处 `is_absolute()` 改为 `starts_with('/')`（Windows 宿主打包前置条件） |
| `crates/skyward-block/src/fstab.rs` | 扫描 `/vendor/etc`、`/odm/etc` 下全部 `fstab.*`，识别 `fstab.O1_asic` |
| `crates/skyward/src/services.rs` | `optional` 服务 ready 超时只记日志、不终止 recovery |
| `crates/skyward/src/main.rs` | shared secret 协商失败只记日志 |
| `crates/skyward/src/ui.rs` | `normal_max()` 无 `msm_drm` 时回退；`run()` 传 refresh/reversed；另外强制 `dual_boot=false` |
| `crates/skyward/src/input.rs` | `swapped` 自动轴对调 + `reversed=["y"]` 镜像，按 0.2.0 的 `Panel`/`Inputs` 结构重打 |
| `crates/skyward-ui/src/surface.rs` | 见 9.4 |
| `crates/skyward-install/src/run.rs` / `skyward-systems/src/install.rs` / `skyward-pack/src/main.rs` | 见 9.2 |
| `crates/skyward-bootimg/src/ramdisk.rs` | 见 9.5 |

### 9.4 刷新率：0.2.0 特有的重新设计

`KmsSurface::open(path, refresh)` 按配置选 60Hz 的 `mode`；**并在算出 `mode` 之后把
`preferred` 重绑到 `&mode`**。因为 0.2.0 用 `preferred` 的 blob 上电（`first`）、
唤醒时也切回 `preferred`，再由 `fastest = (mode.vrefresh() != preferred.vrefresh())`
决定是否 `to_fastest()`。重绑后：上电即 60Hz、`fastest` 恒为 `None`、
`to_fastest()`/`speed_up()` 都成为空操作、唤醒也回 60Hz，不会再出现 144Hz 瞬间。
只改 `mode` 选还不够，这是本次相对旧补丁的关键差异。

### 9.5 新踩的坑：0.2.0 官方打包器在 Windows 宿主编译不过

`skyward-bootimg/src/ramdisk.rs` 的「把 ramdisk 解包到文件夹」用了
`std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt}`、
`OsStrExt::as_bytes`、`symlink`、`lchown`、`mknod`、`utimensat`，
Windows 上直接报 15 个编译错误。上游只在 macOS/Linux 上构建打包器。
WSL 在本环境被安全策略禁用，因此改为 `#[cfg(unix)]` 隔离：

- `c_path()`、`unpack()` 自由函数、`set_time()`、`Ramdisk::unpack` 的 Unix 实现
  及对应的 unix-only imports 全部加 `#[cfg(unix)]`；
- Windows 下 `Ramdisk::unpack` 提供返回明确错误的 stub；
- 测试模块改为 `#[cfg(all(test, unix))]`。

打包器只用到 `decode()` / `encode()` / `add()` / `link()` / `replace()` / `find()`，
这些是纯内存操作 + 外部 `lz4` CLI，Windows 上完全可用。`tools/lz4.exe`
从旧工作区只读复制过来，打包时需加入 PATH。

### 9.6 构建命令（本构建）

```sh
cargo build --release -p skyward-pack
cargo ndk -t arm64-v8a --platform 30 build --release -p skyward
# 不需要 skyward-system-init；--system-init 不传
PATH="$PWD/tools:$PATH" target/release/skyward-pack.exe recovery \
  --stock "<4.0.5>/001_DNA/recovery.img" \
  --binary target/aarch64-linux-android/release/skyward \
  --config devices/violin.toml \
  --font resources/fonts/MiSansVF.ttf \
  --mono resources/fonts/KirkMono-Regular.ttf \
  --mono-license resources/fonts/KirkMono-OFL.txt \
  --out out/skyward-violin-OS4.0.5-$(date +%Y%m%d-%H%M).img
```

### 9.7 刷机后第一轮要看的三件事

1. 开机第一屏到 Skyward 是否不花屏（重点验证 9.4 的上电/唤醒路径）；
2. 触摸是否仍然准确（`swapped` + `reversed=["y"]` 在 0.2.0 的 `Panel` 上重打过）；
3. data 能否解密、开机是否不再等满 40 秒（optional 服务）。

WLAN 与双系统这一轮不测：前者未配置，后者页面不可达且镜像无启动程序。

### 9.8 构建与自检结果（本机完成，未做真机验证）

| 项目 | 结果 |
|---|---|
| `skyward-pack.exe`（宿主） | 编译成功，RC=0，仅 4 个 dead_code 警告 |
| `skyward`（aarch64-linux-android） | 编译成功，RC=0，`8,387,872` 字节 |
| `skyward-pack stock --config devices/violin.toml` | RC=0，解析出的 version / package / recovery 与 4.0.5 一致 |
| 打包 | RC=0，`out/skyward-violin-OS4.0.5-20261112-1811.img`，`104,857,600` 字节（整分区），ramdisk `48,911,186` 字节 |
| **镜像 SHA-256** | `ce39fed6be5fc992e38314349ce821e6a2a6019171b1e6c604563ae5f177101c` |
| 文件名规则 | `skyward-violin-<ROM 版本>-YYYYMMDD-HHmm.img`，时间取镜像**实际生成时刻**（本机 `date -r <img>` 得到 `2026-11-12 18:11:25`）；构建命令里用 `$(date +%Y%m%d-%H%M)` 自动生成，便于每一版区分 |
| ramdisk 往返自检 | `repack` 后 `list` 与成品逐条一致，1047 条全部相同 |
| 代码提交 | `673e1c8 port: violin on skyward 0.2.0 / HyperOS OS4.0.5.0, no WLAN, no second system`（基线 `d6508e8`） |

镜像内容自检：

- `system/bin/recovery` = `8,387,872` 字节，与本地编译产物一致；
- `skyward/device.toml`、`MiSansVF.ttf`、`KirkMono-Regular.ttf`、`KirkMono-OFL.txt` 齐全；
- `system/bin/{7za,avbctl,resetprop,unzip,zstd}` 5 个命令链接齐全（`unzip` 原为 stock 链接，已被重指）；
- `system/etc/task_profiles.json`、`system/etc/init/skyward-mtp.rc` 已补；
- `prop.default` 末尾追加了 `ro.secure=0` / `ro.adb.secure=0` / `ro.debuggable=1` /
  `ro.ota.allow_downgrade=true`（init 以后出现的行覆盖原厂值，这是官方打包器的做法）；
- **`skyward/system-init` 不存在**（双系统无启动程序）；
- **镜像内配置没有 `[wlan]` 段**，`[modules].load` 只有 4 个触摸模块，
  `[partitions].mounted = ["persist"]`。

构建过程中另外两个环境坑（已绕过，代码里不需要留补丁）：

- `cargo ndk ... -p skyward` 在本机挂起 16 分钟、无 rustc 子进程，与 MSVC 路径有关；
  改用 `cargo build --release --target aarch64-linux-android -p skyward`，
  显式设置 `CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER` / `_AR` 和
  `CC_aarch64_linux_android` / `AR_aarch64_linux_android` 指向 NDK r29 的
  `aarch64-linux-android30-clang.cmd` / `llvm-ar.exe`，1 分 12 秒编译完成。
- `rustup target list --installed --toolchain stable-x86_64-pc-windows-gnu`
  会触发该工具链的在线组件同步，本环境不适合使用 GNU 工具链；
  默认 `stable` (MSVC) 用于宿主打包器没有问题。

**本轮没有连接设备、没有刷机、没有写入任何设备分区**，旧工作区
`C:\Users\shc\Desktop\skyward` 全程只读（只复制出 `tools/lz4.exe` 并读取旧补丁/配置）。
镜像需要由你手动刷入后，按 9.7 的三项抓 log 验证。

### 9.9 去掉重启页的 EDL 选项（用户反馈，2026-11-12 18:40 重建）

**反馈**：Xiaomi Pad 7S Pro（玄戒 O1）没有 EDL 模式，重启页里的
「重启进 EDL」是高通设备专用的，必须删掉。

**证据**：`skyward-ui/src/app.rs:1113` 的 `TARGETS` 硬编码 6 项，含
`Target::Edl`；`skyward-ui/src/pages.rs:149` 的 `REBOOT_TARGETS` 里对应文案
直接写着 `["EDL"; 3]` / `高通紧急下载模式` / `Qualcomm emergency download`。
两个数组靠同一个 `state.reboot` 下标一一对应，渲染用
`(0..TARGETS.len())` 生成 `Action::RebootTarget`。

**改动**（4 个文件）：

| 文件 | 改动 |
|---|---|
| `crates/skyward-ui/src/app.rs` | 删 `Target::Edl` 变体，`TARGETS` 改为 5 项并注明原因 |
| `crates/skyward-ui/src/pages.rs` | `REBOOT_TARGETS` 删掉 EDL 行，类型改为 `[...; 5]` |
| `crates/skyward/src/ui.rs` | `power()` 的映射删掉 `Target::Edl => Power::Edl` 分支 |
| `crates/skyward-ui/src/search.rs` | 重启页搜索关键词去掉 `"edl"` |

索引不会错位：`state.reboot` 初始为 0，`TARGETS` 与 `REBOOT_TARGETS` 都是 5 项，
`picks` 由 `TARGETS.len()` 生成。

**保留的边界**：`skyward-sys::Power::Edl` 与 `reboot,edl` 的底层命令解析、
CLI 的 `reboot edl` 说明都保留——那是通用底层能力，UI 已不可达，
设备不会被带进一个不存在的模式。二进制里 `高通紧急下载模式` 与
`Qualcomm emergency download` 两段文案均已消失。

**第二次构建与自检**：

| 项目 | 结果 |
|---|---|
| 重编译 | RC=0，`skyward` `8,387,552` 字节（比上一版少 320 字节） |
| 新镜像 | `out/skyward-violin-OS4.0.5-20261112-1840.img`（实际生成 `2026-11-12 18:40:39`） |
| **新镜像 SHA-256** | `60bf753ac92c6d4aac3513977fc2c8b2bccb49f4b3bf9397724e61f581dd9278` |
| ramdisk | `48,911,033` 字节；`repack` 往返 1047 条逐条一致 |
| 关键项 | `system/bin/recovery` = `8,387,552`；无 `skyward/system-init`；镜像内配置无 `[wlan]` |

**版本对照**：

| 文件 | 说明 |
|---|---|
| `...-20261112-1811.img` | 初版（含 EDL 选项），哈希 `ce39fed6…101c`，保留作历史 |
| `...-20261112-1840.img` | **当前应刷入的版本**（已去掉 EDL），哈希 `60bf753a…9278` |

本轮只做了本机编译与打包，没有刷机、没有写入任何设备分区。

### 9.10 bootloader / fastboot 能力实测（2026-11-12 18:56，纯只读）

设备 `<设备序列号>` 重启到 bootloader 后，只执行了 `devices`、
`getvar` 单项、`oem device-info`、`flashing get_unlock_ability`、`fetch`
这些只读命令；**没有 flash、erase、reboot、boot、解锁或任何写操作**。

**根因**：`getvar version` = `EDK2-XRING 1.0 user`。玄戒 O1 用的是
EDK2/UEFI 风格 bootloader，不是高通 LK/aboot，所以 fastboot 命令集是裁剪过的，
`getvar all`、多数标准变量、`boot`、`fetch` 都缺失。

**实测可用**

| 命令 / 变量 | 结果 | 意义 |
|---|---|---|
| `fastboot devices -l` | `fastboot` | 连接正常 |
| `getvar product` / `variant` | `violin` / `XR_UFS` | 设备与存储类型正确 |
| `getvar version` | `EDK2-XRING 1.0 user` | 能力裁剪的原因 |
| `getvar unlocked` / `oem device-info` | `yes` / `Device unlocked: true` | 可刷未签名镜像 |
| `getvar is-userspace` | `no` | 在 bootloader fastboot，不是 fastbootd |
| `getvar current-slot` / `slot-count` | `b` / `2` | 当前槽 b，A/B 双槽 |
| `getvar has-slot:recovery` | `yes` | recovery 带槽后缀 |
| `getvar partition-size:recovery_b` | `6400000`（十六进制 = 104,857,600） | **与镜像大小精确一致** |
| `getvar partition-type:recovery_b` | `raw` | 物理分区 |
| `getvar is-logical:recovery_b` | `no` | 非动态分区，可直接 flash |
| `getvar max-download-size` | `536870912`（512 MiB） | 远大于 100 MiB 镜像 |
| `getvar anti` | `1` | 防回滚版本 |
| `slot-successful:a` / `:b` | `yes` / `yes` | 两槽都可启动 |
| `slot-unbootable:a` / `:b` | `no` / `no` | 两槽都健康 |
| `fastboot reboot recovery` | 用户真机实测 OK，已成功进入 Skyward | 可直接进 recovery，无需绕道系统 |

**实测不可用**

| 命令 / 变量 | 返回 |
|---|---|
| `fastboot getvar all` | `FAILED (remote: Something went wrong when looking up the variable)` |
| `fastboot flashing get_unlock_ability` | `FAILED (remote: unknown command)` |
| `fastboot fetch recovery_b <file>` | `Device does not support fetch command`（无法从设备读回分区备份） |
| `fastboot boot ...` | 用户已知不可用（与 EDK2 裁剪一致） |
| `partition-size:recovery`（无后缀） | 失败；设备上只有 `recovery_a` / `recovery_b` |
| 其余未实现的标准变量 | `serialno`、`hw-revision`、`board`、`version-bootloader`、`version-baseband`、`secure`、`secure-boot`、`slot-suffixes`、`off-mode-charge`、`battery-voltage`、`cpu-abi`、`treble-enabled`、`first-api-level`、`super-partition-name`、`dynamic-partition`、`download-size` |

**对刷机的实际影响**

1. 没有 `fastboot boot` → **不能临时启动镜像做无落地试刷**；刷 recovery 就是一次性写入。
2. 没有 `fastboot fetch` → **不能从设备读回当前 recovery 备份**；回退底稿只能用本地
   OTA 解出的 `001_DNA/recovery.img`（SHA-256 `376631cf…30293`）。
3. 但 `a` / `b` 两槽都健康，且只需刷 `recovery_b`，**保留 `recovery_a` 原厂版作为第二重退路**。
4. 探测阶段 `flash` 未实测（写操作不能为测试而刷）；随后用户已实际刷入并成功
   进入 Skyward，`flash recovery_b` 与 `reboot recovery` 都已在真机验证可用。

**实际采用并验证的刷法**（用户 2026-11-12 执行）

```sh
fastboot flash recovery_b out/skyward-violin-OS4.0.5-20261112-1840.img
fastboot reboot recovery    # 本机实测可用
```

更正：旧工作区 `HANDOFF-SKYWARD-VIOLIN.md.jpg` 里「`fastboot reboot recovery`
不可用」是 4.0.0.14 时代的记录，不适用于当前 4.0.5 的 EDK2 bootloader；
以真机实测为准。

`has-slot:recovery=yes` 时 `fastboot flash recovery <img>` 也会自动映射到当前槽
（旧工作区 violin 用过），但显式写 `recovery_b` 最不容易刷错。
**不要**加 `--slot all`、不要刷 a 槽、不要碰 `boot`/`init_boot`/`vendor_boot`/
`vbmeta*`/`bootloader*`/`abl`/`xbl`/分区表，不要 `erase`/`format`/
`flashing lock|unlock`/`update`/`flashall`/`wipe-super`。
只刷 recovery 不会造成硬砖；最坏是 recovery 起不来，用原厂镜像刷回即可。

### 9.11 真机首轮验证（2026-11-12 19:01，只读抓 log）

用户用 `fastboot reboot recovery` **成功重启进入最新构建的 Skyward**，命令实测可用；
旧交接文档里「不可用」的结论已按真机结果更正（见 9.10）。

日志归档：`logs/20261112-1901/`（`kernel.log` 7830 行、`props.txt`、
`mounts.txt`、`cmdline.txt`、`input-devices.txt`、`backlight.txt`）。
**0.2.0 不再写 `/tmp/recovery.log`**，Skyward 自己的日志以 `skyward:` 前缀
进入内核日志，所以从 `kernel.log` 里提取即可。抓 log 时注意 Git Bash 会把
`/tmp` 参数转换成 Windows 临时路径，需要把整条命令用引号包起来。

**三项重点全部通过**

| 验证项 | 证据 |
|---|---|
| 显示不花屏 | `dpu_crtc_print_commit_info: new refresh rate is 60(hz)`；息屏 `(On)→(Off)` 再亮屏 `(Off)→(On)` 后仍是 `60(hz) old 60(hz)`；**全程无 `underflow`、无 144Hz**。9.4 把 `preferred` 重绑到 60Hz mode 的做法在真机上生效 |
| 触摸链路 | 4 个触摸模块 + 依赖全部加载；`touchfeature-service` 成功注册 `vendor.xiaomi.hw.touchfeature.ITouchFeature/default`；`enable touch raw 1`；`skyward: wrote 1 to /sys/class/touch/touch_dev/enable_touch_raw`；`/dev/input/event0..event5` 存在 |
| data 解密 | `/dev/block/dm-10 /data f2fs rw ... inlinecrypt` 已挂载；`skyward: 1 of the secure world agreed on the key it signs its notes with`；keymint / gatekeeper / tee-supplicant 会话都打开成功；开机没有再等满 40 秒 |

`skyward: violin 0.2.0 recovery up, misc command "" args [], asking for Menu`
确认 0.2.0 主程序已进入菜单。`ro.secure=0` / `ro.adb.secure=0` /
`ro.debuggable=1` 生效，adb 免授权可用。日志里没有 WLAN 或双系统相关行，
与「两项都未构建」一致。

**两个非致命观察**

- `skyward: recovery failed: turn the display off: Invalid argument (os error 22)`
  出现在启动早期（107.443 s），紧接着 107.547 s 有一次成功的 `recovery up`，
  当前实例运行正常。疑似 0.2.0 新增的息屏/LP 属性路径在玄戒 DPU 上返回
  EINVAL，值得下一轮确认是否每次开机都出现。
- `skyward: logd not started: logd.ready is Some("false") after 5000 ms` 与
  `clock not set: read persist's time/ats_2: No such file or directory`：
  自带 logd 未就绪、时钟未从 persist 恢复，只影响日志/时间显示，不影响功能。

**仍需人工实测**：触摸**坐标**是否准确（`swapped` + `reversed=["y"]` 在 0.2.0
的 `Panel` 上重打过）。链路已通，但坐标准不准需要在屏幕上点几个位置确认，
log 本身看不出来。

### 9.12 锁屏崩溃 / 旋转：上游 PR 调研（2026-11-12，只读）

#### PR #30 `fix: 定时重写触摸设置，休眠回来触摸不再失效`（open，基于 main ff1668f）

只改 `crates/skyward/src/input.rs`（+47）：新增 `TOUCH_REFRESH = 1s` 的后台线程，
每秒把 `[touch].writes` 重写一遍。原因是小米驱动在 `xiaomi_touch_resume_work`
里把触摸控制器恢复成上电状态，而 `/dev/input` 节点没变化，`scan()` 的
`seen` 集合不会让它重跑，于是首次灭屏后 `enable_touch_raw=1` 就丢了，
亮屏后触摸失效。

- **不能修我们的崩溃**：完全不碰 `surface.rs` / `turn_off()`。
- **但必须一起做**：violin 用的正是小米触摸驱动（log 里有
  `store_enable_touch_raw: enable touch raw 1`）。修好崩溃后正常息屏/唤醒
  不再重启进程，`scan()` 不会重跑，这个重置问题几乎必然会暴露。
- 补丁位置（`use` 区 + `Inputs::open` 末尾）与我们重打过的 `input.rs`
  不冲突，可直接套。

#### 方案 A 是否仍需做：**需要**

四个 PR 里没有任何针对玄戒 `xring_drm` 在 `ACTIVE=false` 上返回
`EINVAL(22)` 的修复。PR #44 的 `surface.rs` 改动是给**高通 video-mode 面板**
（`mode.name().ends_with("vid")`）修开机 splash 的，甚至主动调用 `turn_off()`，
套到 violin 的 cmd-mode 面板只会更糟。所以方案 A 仍是唯一对症的修法，
且应与 PR #30 的定时重写一起做——两个不同的 bug，同一条息屏路径。

#### 旋转：PR #25 / #44 / #24

| PR | 状态 / 基线 | 关系 |
|---|---|---|
| #24 | closed，基于 v0.1.6 (`3c5b0a2`)，中文 | #25 的早期版 |
| #25 | open，基于 v0.1.6，英文 | 同一功能（Pad 6S Pro `sheng`） |
| #44 | open，**基于 main ff1668f（与我们的 0.2.0 基线一致）** | 最新、含前两者 + Pad 7 Pro `muyu` |

通用能力（可复用）：`[device] tablet = true` / `supports_vibration`、
`skyward-ui/src/tablet.rs`（分栏布局）、`skyward-ui/src/rotation.rs`
（`Gravity` 滤波、`Rotation` 枚举、约 420ms 旋转动画）、配置里的
`[touch] swap_xy / flip_x / flip_y / ignored_devices`、`[orientation]`。
`muyu.toml` 写的是 `swap_xy = true`、`flip_y = true`，**与我们真机实测推出的
`swapped` + `reversed=["y"]` 完全一致**，互相印证了坐标映射结论。

**关键限制**：自动旋转的方向源 `skyward/src/orientation.rs` 开头明确写着
「Nonblocking Qualcomm SSC client」，走 QMI `sns_client_api_v01` / SSC
服务号 400 / accel protobuf / sensor-firmware 元数据。violin 是玄戒 O1，
**没有高通 SSC**，这个后端不可用。要么另找玄戒的加速度传感器接口，
要么先做**固定方向**（复用 `rotation.rs` 的渲染旋转 + `tablet.rs`，方向写死）。

**不能直接整体合并**：#44 有 32 个文件、约 2500 行，混入了高通专属内容
（`qseecomd`、`[firmware] remoteproc`、SSC、`muyu`/`sheng` 设备配置、
高通 WLAN），且与我们已经改过的 `input.rs` / `ui.rs` / `surface.rs` /
`skyward-config` 有重叠，只能挑通用部分手工移植，建议单独一轮做。

**建议顺序**：① 方案 A + PR #30 等价补丁 → 重编译/打包/刷入 → 验证锁屏；
② 触摸坐标人工实测；③ 旋转单独一轮（先定「自动」还是「固定横屏」，
再决定移植范围）。

### 9.13 全部 PR 盘点（32 个，2026-11-13）

`pulls?state=all` 共 32 个（open 24 / closed 8，无第 2 页）。逐个看了标题、
正文、改动文件；重点 PR 读了 patch。

#### 对 violin 有用的（按优先级）

| 级别 | PR | 改动 | 判断 |
|---|---|---|---|
| **P0** | #30 | `input.rs` +47，每秒重写 `touch.writes` | 见 9.12，修唤醒后触摸失效，与方案 A 同一条息屏路径 |
| **P0** | #28 | `skyward-bootimg/src/ramdisk.rs` +18 -1 | `encode()` 在找不到外部 `lz4` 时回退到内建 `lz4_legacy`。我们靠复制 `lz4.exe` 绕过，合入后可去掉这个外部依赖；有 `lz4` 时行为不变，**零风险** |
| **P0** | #35 | `skyward-backup/src/lib.rs` +1、`skyward-ui/src/settings.rs` +4 | v0.2.0 备份 manifest 多了第 5 部分 `firmware`，上游仍是固定 4 元数组。直接影响备份页/设置持久化，**必合，改动极小** |
| **P0** | #22 | `skyward-block/src/mount.rs` +6 | `unpark` 跳过已不是挂载点的路径，否则 `remount(MS_SHARED)` 返回 **EINVAL(22)**。与我们息屏 EINVAL 同源不同位置，**零风险** |
| **P1** | #31 | `mount.rs` / `vab/partitions.rs` / `secure.rs` +66 | 刷包时 loop 设备占住分区导致卸不掉，安装失败。用户会刷 OTA，值得合 |
| **P1** | #33 | `skyward-crypto/src/data.rs` +49 -42 | 遍历用户所有 protector（weaver 放最后），而不是只试第一个。解密健壮性 |
| **P1** | #47 | `skyward-mtp/{service,usb}.rs` +78 | Windows/WPD 完成事件丢失、suspend/resume 后重来、断开时正确返回。我们用 Windows 传文件 |
| **P2** | #11 | `skyward-backup/src/archive.rs` +358 | 修二进制 PAX 扩展属性导致的数据恢复失败，并在恢复前校验归档。改动大，按需 |
| **P2** | #29 | `ramdisk.rs` +14、`skyward-pack/main.rs` +59 | 打包器补入 stock ramdisk 缺的库。会碰到我们改过的 `--system-init`，需手工合并 |
| **P3** | #25 / #44 / #24 / #23 | 平板 UI / 自动旋转 | 见 9.12。`#23` 是更早的 Pad 实验版（closed），含「提前加载触摸固件解决首屏后数秒不能触摸」，可作为触摸冷启动的参考 |
| 参考 | #1 | 大杂烩（root + Pad 8 Pro） | 我们当初的 `swapped` + `reversed` 就参考它；其余（root 管理器、release 下载）不适用 |

#### 重要更正：方案 A 可以做得更准

9.12 里写「四个 PR 没有任何针对 EINVAL 的修复」**不够准确**。全量看过后，
有两处上游经验直接相关：

- **#38**（bixi 折叠屏）`surface.rs` 的 `darken()`：提交 `ACTIVE=false` 前
  先 `crtc_props.value("ACTIVE")? == 0` 就跳过，并写明
  「a display detached from nothing is refused」——**对已经不 active 的 CRTC
  再提交一次会被拒绝**。这正是我们息屏后 `rest()` 已让面板进低功耗、
  5 秒后 `turn_off()` 再提交 `ACTIVE=false` 返回 EINVAL 的合理解释。
- **#25 / #44** `surface.rs` 注释：「**Switching rate and ACTIVE together is
  invalid**」，做法是 `preferred = blob; fastest = None`，避免唤醒时同时切
  刷新率和 ACTIVE。**这与我们 9.4 把 `preferred` 重绑到 60Hz、`fastest=None`
  的做法一致**，等于上游独立印证了该修复。

因此方案 A 升级为：`turn_off()` 先读 `ACTIVE`，已为 0 就幂等返回；
确需提交时按驱动期望处理 connector `CRTC_ID`；提交失败降级为记日志而
**不 `?` 冒泡杀进程**。三道保险，比单纯跳过更稳。

#### 不适用 / 暂不需要

- 设备专属配置：#7 (nuwa)、#26 (mondrian)、#32 (chiron/polaris/raphael)、
  #36 (aurora)、#42 (popsicle)、#43 (socrates)、#48 (myron)、#45 (warhol)、
  #39 (alioth)、#11 的设备部分。
- #43 / #48 的「REC 解密版本兼容」(KeyMint -62/-38)：我们 4.0.5 解密已成功，
  暂不需要；以后遇到密钥版本错配再取。
- #46 `[wlan] remember`：本构建无 WLAN。
- #45 vendor_boot 型 recovery：violin 有独立 `recovery_a/b` 分区。
- #13 / #19 / #12 / #14 / #15 / #27 / #34：安装细节、设置持久化、语言、
  formatter、VINTF 描述解析、单测；当前未遇到对应问题或已被 0.2.0 覆盖。

#### 建议

本轮先合 **P0 四件 + 升级版方案 A**，一次性重编译/打包/刷入验证锁屏；
P1 三件可在验证通过后单独一轮合入，避免多个变量叠加。旋转仍按 9.12
单独一轮（自动旋转依赖高通 SSC，violin 没有）。

### 9.14 P0 实施与重建（2026-11-13）

#### 实际改动

| 文件 | 内容 |
|---|---|
| `crates/skyward-ui/src/surface.rs` | **升级版方案 A**：`turn_off()` 先读 CRTC 的 `ACTIVE`，已为 0 就幂等返回；`atomic_commit(ACTIVE=false)` 失败时只 `eprintln!` 并把 `power` 留在 `Resting`，**不再 `?` 冒泡杀进程**；`is_off()` 改为 `Off \| Resting` 都算，保证仍能进入系统 suspend |
| `crates/skyward/src/input.rs` | **PR #30**：`TOUCH_REFRESH = 1s`，`keep_touch_told()` 后台线程每秒重写 `[touch].writes`，覆盖小米 `xiaomi_touch_resume_work` 在休眠回来后的重置 |
| `crates/skyward-bootimg/src/ramdisk.rs` | **PR #28**：`encode()` 在找不到外部 `lz4` 时回退到内建 `lz4_legacy()`，新增 `no_lz4_tool()` 只把 `NotFound` 当可回退错误 |
| `crates/skyward-block/src/mount.rs` | **PR #22**：`unpark` 跳过已不是挂载点的 privatized 路径，避免 `remount(MS_SHARED)` 的 EINVAL(22) |

**#35 跳过**：它假设 manifest 已有第 5 部分 `firmware`，但本基线
`Manifest::parts() -> [bool; 4]`、`State.backup: [bool; 4]`，而
`picked<const N>() -> [bool; N]`；把 `PARTS` 改成 5 会直接编译失败
（该 PR 自身也未通过 CI）。要支持第 5 部分得连 #38 的
`backup/image.rs` 一起搬，超出本轮范围。

方案 A 的三道保险借鉴了 #38（提交前查 `ACTIVE`，「a display detached
from nothing is refused」）与 #25（「Switching rate and ACTIVE together
is invalid」）。失败后留在 `Resting`，`present()` 唤醒时只做普通 page
flip 和 `LP=ON`，不会触发那次会失败的 `ACTIVE=true` + `MODE_ID` 完整
modeset。

#### 构建与自检

| 项目 | 结果 |
|---|---|
| `skyward-pack.exe` / `skyward` 重编译 | RC=0 / RC=0；`skyward` `8,389,504` 字节 |
| **交付镜像** | `out/skyward-violin-OS4.0.5-20261113-1814.img`（实际生成 `2026-11-13 18:14:09`） |
| **镜像 SHA-256** | `28c61bb0688748f8022420303b2053abcd58637601e0f0bcf5cbebf1f4c78780` |
| ramdisk | `48,912,655` 字节；`repack` 往返 1047 条逐条一致 |
| 镜像内容 | `system/bin/recovery` = `8,389,504`；无 `skyward/system-init`；配置无 `[wlan]` |
| 二进制内字符串 | `keeping the panel resting`、`for the touchscreen again` 均在；`高通紧急下载模式` 仍缺席 |

**#28 内建回退已单独验证**：故意不把 `lz4.exe` 放进 PATH 打包，成功产出
`55,393,917` 字节 ramdisk 的镜像（内建 `lz4_flex` 压缩率低于 `lz4 -12`）。
但该压缩格式**没有上过真机**，而前两版用外部 `lz4 -l -12` 的镜像已真机
验证过；为避免验证锁屏修复时再叠加一个压缩格式变量，测试镜像已删除，
**交付镜像仍用外部 lz4 路径**。`tools/lz4.exe` 继续保留作为构建依赖，
以后也可以完全不要它。

#### 版本对照

| 文件 | 说明 |
|---|---|
| `...-20261112-1811.img` | 初版（含 EDL 选项），`ce39fed6…101c` |
| `...-20261112-1840.img` | 去掉 EDL，`60bf753a…9278` |
| `...-20261113-1814.img` | **当前应刷入的版本**：P0 四件 + 升级版方案 A，`28c61bb0…8780` |

#### 刷机后要验证的四件事

1. 按电源键锁屏 → **不再出现 `recovery failed` / init 重启 recovery**；
2. 锁屏后**触摸屏幕应能直接点亮**，不再要再按一次电源键；
3. 亮屏后**不需要重新解密 data**（`/data` 保持挂载、进程未重启）；
4. 亮屏后**触摸仍有反应**（#30 的每秒重写覆盖了驱动重置）。

抓 log 时直接从 `adb shell dmesg` 里看 `skyward:` 行即可（0.2.0 不写
`/tmp/recovery.log`）。

本轮只在本机编译打包，**没有连接设备、没有刷机、没有写入任何设备分区**。

### 9.15 P0 真机验证（2026-11-13 20:52 只读抓 log）

日志归档：`logs/20261113-2052/`（`kernel.log` 9360 行、`props.txt`、
`mounts.txt`、`cmdline.txt`）。

| 检查项 | 结果 |
|---|---|
| `recovery failed` | **0 次**（修复前每次锁屏 1 次，直接崩进程） |
| init 启动 recovery | 1 次，即开机那一次，没有崩溃后重启 |
| `touch.writes` 每秒重写 | 110 次，PR #30 生效 |
| 锁屏 → 亮屏 | 85.560 s 背光 0 → 87.996 s 背光 410，**2.4 秒**（原来 5–6 秒） |
| 重新解密 | 没有：进程未重启，`/data` 保持挂载，无需再走 bring_up |
| 当前刷新率 | `new refresh rate is 60(hz)`，全程无 underflow |

**但 `turn_off()` 本身仍失败**，只是不再致命：

```
[85.583080] [DPU-E]: dpu_power_check: commit valid plane to off crtc
[85.583099] [DPU-E]: dpu_kms_check: dpu power check failed
[85.583112] [DPU-E]: dpu_crtc_atomic_check: [CRTC:0][frame_no:0]: ret -22
```

玄戒 DPU 拒绝「在关闭 CRTC 的 commit 里还带着有效 plane」。
PR #25/#44 的 `restore` 逻辑正是为此：关闭时把 `MODE_ID` 置空、断开
connector 的 `CRTC_ID`、并清空每个 plane 的 `FB_ID`/`CRTC_ID`；唤醒时再
用保存的完整 req 重建 pipeline。它目前只在
`mode.name().ends_with("vid")` 的 video panel 上启用——而 violin 的
lockdown info 明确是 `dualdsi dsc lcd video panel`，属于 video panel，
只是 mode name 未必以 `vid` 结尾。这是下一步真正消掉 EINVAL、
把唤醒做到毫秒级的方向（当前降级为 2.4 秒且需再按电源键，触摸点亮
仍待用户确认）。

### 9.16 144Hz 刷新率：根因与可行路径（2026-11-13）

**面板实际支持 7 档**（`/sys/class/drm/card0-DSI-1/modes`）：
`3200x2136x120 / 144 / 90 / 60 / 50 / 48 / 30`。

**锁定信息**：`[xiaomi o81a 36 02 0a dualdsi dsc lcd video panel-0]`
——双 DSI + DSC + video panel；`is_cmd_mode:0`。

**根因（有硬证据）**：从 ROM 里设备实际运行的
`vendor_boot/.../xring-drm.ko`（6.18.21）提取字符串：

| 高通 SDE 属性（recovery 在用） | xring-drm.ko 里 |
|---|---|
| `mode_properties` blob（`SdeMode::find` 据此定 mixers/DSC） | **不存在** |
| `frame_trigger_mode` / `posted_start` | **不存在** |
| `SDE_SPR_INIT_CFG_V2` | **不存在** |
| `dsc_mode` enum | 只有 DP 相关的 bad_dsc_mode 符号 |

玄戒自己的东西：`dscr_blob_prop`、`dscw_blob_prop`（DSC blob 属性）、
`vote ddr %u MB/s`（DDR 带宽投票）、`frame_power_max_refresh_rate` /
`current_refresh_rate`、`144fps_times`、`video underflow: %d`。

因此 `surface.rs::open()` 里 `conn_props.get("mode_properties")` 必然失败，
fallback 成 **`SdeMode { mixers: 1, dsc: false }`**：单 pipe、不开 DSC、
也不做玄戒那套 DSC blob 与带宽投票。60Hz 带宽够所以正常，
144Hz 带宽不够 → DPU underflow → 花屏。这不是「刷新率选错」，
是**用高通 SDE 的配置方式去驱动玄戒 DPU**。

**能修，但要分步，且随时可回退**（`refresh = 60` 一行即可退回）：

1. **零代码风险，先摸门槛**：面板有 120/90 档。把 `devices/violin.toml`
   的 `refresh` 依次改成 `120`、`144`，只重新打包（不需重编译），刷入看
   是否花屏。若 120 正常、144 花屏，说明门槛就在 DSC/带宽上，也给出
   一个比 60Hz 更流畅的可用档位。
2. **拿真实属性清单**：设备重新连上后跑只读的
   `/system/bin/recovery kms`（自带命令，打印 connector/CRTC/plane 属性、
   `mode_properties`/`capabilities` blob、全部 mode 的 clock/flags）。
   这次抓时设备掉线没拿到。注意 Git Bash 会转换 `/dev/...` 路径，
   要用 `MSYS_NO_PATHCONV=1` 或不传 `--card`（默认已是 `/dev/dri/card0`）。
3. **按玄戒属性补 DSC + 双 pipe**：在 `KmsSurface::open()` 里增加一个
   非高通分支，按 kms 报告里的属性设置 DSC blob（`dscr_blob_prop` /
   `dscw_blob_prop`）与 2 个 mixer 的 plane 布局，必要时补 DDR 带宽投票。
   改动集中在显示初始化，风险中等，必须保留 60Hz 回退开关。
4. **息屏 EINVAL 一起修**：按 9.15 的 #44 `restore` 思路，关闭 CRTC 时
   清空所有 plane 绑定，唤醒时重建完整 pipeline。

**不建议**直接把 `refresh` 改成 144 就当修好：那只会重现
`dpu_underflow_work`，且会同时叠加息屏/唤醒路径的变化。

本轮均为只读诊断：抓 log、读 sysfs、解 ROM 内的 `.ko` 字符串；
**没有改代码、没有刷机、没有写入任何设备分区**。

### 9.17 144Hz 测试镜像 / 触摸点亮 / 服务内置（2026-11-13 21:03）

#### 1. 144Hz 测试镜像（已产出）

用户指出 HyperOS 里只有 **60 / 90 / 144** 三档，没有 120，所以跳过 120
直接试 144。用独立配置副本 `.references/violin-144hz.toml`（只把
`refresh` 改成 144）打包，**主配置 `devices/violin.toml` 仍是已验证的
60Hz**，随时可回退；配置以 `skyward/device.toml` 原样写进 ramdisk。

| 项目 | 值 |
|---|---|
| 镜像 | `out/skyward-violin-OS4.0.5-144hz-20261113-2103.img` |
| 生成时间 | `2026-11-13 21:03:24` |
| SHA-256 | `23585a2009760c8724cb1c7b88a94f184d56772061e7550704f8cc891913e236` |
| ramdisk | `48,912,657` 字节；镜像内已核对 `refresh = 144` |
| 文件名 | 测试版带 `-144hz-` 标记，与稳定版区分 |

二进制与 1814 版完全相同，只换了设备描述。若花屏，刷回
`...-20261113-1814.img` 即可。按 9.16 的判断，144Hz 大概率仍会
underflow（单 pipe + 无 DSC）；这版是用来**实测确认门槛**的。

#### 2. 息屏后触摸不亮屏：上游有意为之，能修但要改设计

`skyward-ui/src/app.rs::touch()` 第一行：

```rust
// A finger on the dark panel lights nothing, as on the phone's own
// system, where the power key lights it.
if !self.display_on {
    return Vec::new();
}
```

**不是驱动问题，是上游明确的设计**：暗屏时触摸事件被直接丢弃，
只有电源键（以及插线 `plugged()` → `wake()`）能点亮。
`wake(now)` 已存在，会把 `display_on = true`、`curtain` 拉起、
`stir()` 重置息屏计时，所以 UI 层改动很小。

但只改 UI 不够，还有两层：

- **触摸控制器是否还在上报事件**：`surface.rs::rest()` 的注释明说
  「A touch controller that follows the panel, as Xiaomi's do, rests with
  it either way」——息屏设 LP2 后小米触摸控制器会一起休息。需要让
  violin 息屏时**只关背光、不进 LP2**（或确认 LP2 下仍能上报）。
- **内核 suspend**：`skyward-sys/sleep.rs` 的注释只列了
  「a key, the charger, an alarm or the network」作为唤醒源，没提触摸。
  若系统已 suspend，触摸设备还需要是 wakeup source，或干脆禁止
  `may_sleep`（recovery 场景功耗代价可接受）。

建议的最小改法：加一个设备配置项（如 `[touch] wake = true`），
在 `App::touch()` 暗屏分支里改为 `self.wake(now); return Vec::new();`
（第一次触摸只点亮、不误触页面），同时 `ui.rs` 息屏时跳过
`surface.rest()` / `turn_off()` 并强制 `may_sleep = false`。
**需要真机验证**：设备掉线期间无法确认 LP2 下触摸是否还有中断。

#### 3. 把服务二进制内置进镜像：技术上可行，但不解决「刷机报错」

先澄清一个可能的误解：**`fastboot flash recovery` 只写 recovery 分区，
不会挂载 system/vendor/odm，因此刷机时不会因为服务文件在分区里而报错**。
挂载是 recovery 运行时 `bring_up()` 做的事；你此前多次刷入都成功了。

现状核对：

- 4.0.5 ROM 里**没有** system 下的独立 `keymint@3.0-service` /
  `gatekeeper-service` 二进制，只有 `gatekeeperd` 和一堆 `.so`；
  我们配置里用的 5 个都是 vendor/odm 的明确路径，是对的。
- 相关二进制合计约 **1.15 MiB**，ramdisk 48.9 MiB / 100 MiB，
  空间完全够。
- **stock recovery ramdisk 本来就自带**
  `system/bin/hw/android.hardware.boot-service.xring.recovery`（136 KB）
  和它的 rc / VINTF manifest，这些已经在我们的镜像里了；
  我们目前没用它，而是去 `/vendor/bin/hw/` 找。

真要内置，机制现成：**PR #29** 给 `skyward-pack recovery` 加了
`--extra <目录树>`，把整个目录按相对路径塞进 ramdisk（含符号链接、
Windows 下 `/bin/` 文件自动给 0755），并把 `[[service]].path` 改成
ramdisk 内路径即可。我们已改过 `ramdisk.rs` / `skyward-pack/main.rs`，
手工合并不难。

**但不建议为了「不挂分区」而做**，因为仍然离不开分区：

| 依赖 | 来源 |
|---|---|
| 动态库 `/vendor/lib64`、`/system/lib64`、`/odm/lib64`、`/mnt/system/system/lib64/bootstrap` | 分区（`LD_LIBRARY_PATH`） |
| VINTF manifest `/vendor/etc/vintf/manifest`、`/odm/etc/vintf/manifest` | 分区，`secure::publish_descriptions()` 要读 |
| VNDK bundle（`secure::libraries()`） | 分区 |
| mitee TEE 相关配置 | 分区 |

而且内置会带来**版本耦合**：这些服务与系统 VNDK / manifest / TEE 固件
配套，系统 OTA 升级后 recovery 里的旧二进制可能与新系统不匹配；
从分区读取反而自动跟随系统版本。

结论：**可以为「个别易缺失的小文件」用 #29 的 `--extra`（比如 recovery
linker 找不到的库），但整套服务内置既不必要也不更稳**。若确有具体报错
（例如某次刷机提示找不到某个服务），把那条日志发我，针对那一个文件处理。

本轮只新增了一个 144Hz 测试镜像与一个配置副本，**源码一行未改，
没有刷机、没有写入任何设备分区**。

### 9.18 144Hz 真机结果：花屏，根因精确到「玄戒 video-mode 的 present + DDR vote 链路」（2026-11-13 21:09）

用户刷入 `skyward-violin-OS4.0.5-144hz-20261113-2103.img` 后**确认花屏**。
设备仍在 recovery 且 adb 可用，只读抓 log 归档在 `logs/20261113-2109/`
（`kernel.log` 10266 行、`props.txt`、`cmdline.txt`）。
`kms` 只读报告本次仍没抓到：不带 `--card` 返回 RC=255，随后设备掉线；
**没有重试、没有擅自重启设备**。

#### 与 60Hz 归档的硬对比

| | 60Hz（`logs/20261113-2052`） | 144Hz（`logs/20261113-2109`） |
|---|---|---|
| `underflow` 出现次数 | **0** | **903** |
| 典型行 | `dpu_crtc_print_commit_info: new refresh rate is 60(hz)` | `dpu_underflow_work: pipe 0, DSI-1(frame_no 0): underflow, vrefresh_rate 144(hz)` |
| 帧计数 | 正常 | 始终 `frame_no 0` |
| 结束状态 | 正常显示 | `[59.099] check_underflow_occur: underflow occur and not handler, return` |

`UNDERFLOW suppressed (count=578→582)` 说明不是偶发一次，而是持续刷不出来。

#### 决定性证据（45.576 s 的 DPU perf dump）

```
do_dpu_core_perf_dump: last vote: ddr 0 MB/s, clk profile 1
do_dpu_core_perf_dump: real time: ddr 40958 MB/s, real core clk 483333333, axi clk 557056000
dpu_ddr_dvfs_ok_dump: ddr dvfs ok status: ... last_freq=836
dpu_idle_status_dump: vote: vote_count: none
dpu_idle_status_dump: display_0: enable=1 power=1 policy=0x6 mode=video fps=144
dpu_idle_status_dump: display_0 trace: present=0@41647323us ...
dpu_virt_pipe_sw_start_dump: pipe0 sw_start: frame_no=0 time=0us
dpu_hw_mixer_status_dump: [mixer0] comps_wh(0xc80, 0x858)   # 3200x2136 单 layer 全屏
dpu_hw_rdma_path_parse_dbg_irq: [rdma_path0] tlb miss
```

从设备实际运行的 6.18 `xring-drm.ko` 里能对应到内部符号：
`dpu_core_perf_prepare` / `dpu_core_perf_finish`、`dpu_hw_dvfs_vote`、
`veu_ddr_bandwidth_update`、`DSC_THROUGHPUT_TABLE`、`SW_START_OFFSET`、
`MI_DISP_IOCTL_GET_FPS`，以及 `90/120/144fps_times` 帧率跟踪。

**结论（修正 9.16 的笼统说法）**：不只是「单 pipe + 无 DSC 带宽不够」。
这块面板是 `dualdsi dsc lcd video panel`，玄戒 DPU 在 video-mode 下要求
一次真正的 **present**（`present=0` → 驱动认为软件从未送帧）来触发
`sw_start`（`frame_no=0`）并做 **DDR 带宽投票**（`vote_count: none`、
`last vote: ddr 0 MB/s`，而实测需求 `40958 MB/s`）。60Hz 需求约 17 GB/s，
DDR 默认频率（`last_freq=836`）扛得住，所以无 vote 也不花屏；144Hz 需要
约 40 GB/s，没有 vote 就必然 underflow。Skyward 用标准 DRM atomic commit
提交一次静态 framebuffer 后不再提交，高通 cmd-mode 路径够用，玄戒
video-mode 不够。**这是显示后端的平台差异，不是 `refresh` 写错。**

#### 已做的处置

- 主配置 `devices/violin.toml` **仍保持已验证的 `refresh = 60`**，未改动。
- 新增 90Hz 测试镜像（系统官方档位中唯一剩下的中间档，零代码改动）：
  - `out/skyward-violin-OS4.0.5-90hz-20261113-2111.img`
  - SHA-256 `e9b9070373f545551f6e9b41068aafbd1bdb7ae8c2d5b8744cf3abdc692fbf76`
  - 实际生成 21:11:31，镜像内已核对 `refresh = 90`，二进制与 1814 版相同。
  - 配置副本 `.references/violin-90hz.toml`。
- **已验证可回退的 60Hz 镜像**：`out/skyward-violin-OS4.0.5-20261113-1814.img`
  （SHA-256 `28c61bb0688748f8022420303b2053abcd58637601e0f0bcf5cbebf1f4c78780`）。
  刷回命令：`fastboot flash recovery_b <该镜像>` → `fastboot reboot recovery`。
  原厂底稿 `001_DNA/recovery.img`（`376631cf…30293`）也在。

90Hz 的带宽需求约 25.6 GB/s，处在 60 与 144 之间，是否仍触发 underflow
**只能真机实测**；如果 90Hz 也花屏，就证实门槛确实在无 vote 的 DDR 上限，
继续试其它刷新率没有意义。

#### 真要修 144Hz 的前置条件（尚未执行）

1. 设备重新连上后跑只读 `/system/bin/recovery kms`（注意 Git Bash 需
   `MSYS_NO_PATHCONV=1`，或省略 `--card`），拿到玄戒 DPU 暴露的
   connector/CRTC/plane 属性全清单，确认有没有可用的带宽/DSC 私有属性。
2. 进正常系统（设置里选 144Hz）抓一份 DPU 状态作对照：
   `present` 计数、DDR vote 值、mixer 数量、DSC 是否开启。这是官方
   HWC 正确配置的基准。
3. 之后才谈得上改 `KmsSurface::open()`：设置玄戒的 DSC blob
   （`dscr_blob_prop` / `dscw_blob_prop`）、双 pipe 布局、带宽投票，
   并可能需要让渲染循环按 vsync 持续 present。**改动集中在显示初始化，
   必须保留 60Hz 一键回退开关。**

本轮纯只读诊断 + 本地打包：**没有改源码、没有改主配置、没有刷机、
没有写入任何设备分区**；设备最后状态是 adb 掉线（未做任何重启）。

### 9.19 90Hz 真机通过 / adb 反复掉线的根因（2026-11-13 21:14）

用户刷入 90Hz 测试镜像后**确认显示正常**。只读 log 归档在
`logs/20261113-2114/`（`kernel.log` 9383 行、掉线后的增量
`kernel-after-drop.log` 9375 行、`props.txt`、`cmdline.txt`、`drm-modes.txt`）。

#### 90Hz 结论：可用，且把 144Hz 的门槛钉死在 DDR 带宽

```
[15.865463] dpu_crtc_print_commit_info: [CRTC:63][frame_no:0] new refresh rate is 90(hz)
```

`underflow` **0 次**，连 `dpu_core_perf_dump` 都没有触发。三档对照：

| 刷新率 | 估算 DDR 需求 | 结果 |
|---|---|---|
| 60 Hz | ~17 GB/s | 正常，0 underflow |
| **90 Hz** | ~25 GB/s | **正常，0 underflow** |
| 144 Hz | ~40 GB/s（`real time: ddr 40958 MB/s`） | 903 次 underflow、花屏 |

`vote_count: none` 说明三档都没有额外投票，靠的是 DDR 默认频率
（`last_freq=836`）；上限落在 25~40 GB/s 之间。这与 9.18 的结论一致，
也说明**继续试 120Hz 意义不大**（系统 UI 本来也不提供该档）。

#### adb 掉线：**设备侧从未断开，是 Windows 对 MTP 复合设备的重枚举**

本次现场复现：连续 3 次 `adb devices` 全空，紧接着再查又自己回来
（`transport_id:13`），用户描述的「总是掉」属实。

设备侧的硬证据（100 s → 259 s 全段）：

- `sys.usb.state=mtp,adb`、`sys.usb.config_disable=0`、
  `init.svc.adbd=running`、`usb_online:1` 持续为真；
- **没有任何** `xr_usb_composite_disconnect` / `adbd` 重启 /
  `sys.usb.config` 变化 / `mtp: file transfer stops`；
- Windows 侧 `USB\VID_18D1&PID_4EE2` 是**复合设备**：
  `MI_00` = WPD「Xiaomi Pad 7S Pro 12.5」(MTP)，`MI_01` = ADB。
  61.68 s 的 audit 显示 `mtp` 进程正在被 Windows 浏览 `/sdcard`。

结论：`mtp,adb` 把两个 function 绑在同一个 USB 设备上，WPD 一侧被
Windows 后台轮询 / 选择性暂停 / 重置时，整个复合设备重新枚举，
ADB interface 随之瞬时消失；设备侧完全无感，所以日志里查不到断连。
PR #47 的注释正写着「the computer leaves the bus alone for a while and
comes back」，并为此修了 `SUSPEND/RESUME`、`ECONNRESET`、WPD 分块传输，
把 `PORT_SWITCH` 从 5 s 放宽到 15 s。**0.2.0 的 `usb.rs` 里
`BIND | SUSPEND | RESUME => None` 直接忽略挂起事件**；而
`service.rs:96-103` 一旦 MTP 线程出错就 `usb::switch("adb", 5s)`
把端口切走 —— 那是另一条潜在的掉线路径，本次未抓到触发证据。

**唯一一次设备侧重配是开机 16.26 s**（设计内、只发生一次）：
`sys.usb.config=none → mtp,adb`，adbd(pid 343) 被 SIGKILL、以 pid 402
重启，USB 重新枚举后 16.73 s `mtp: the computer opened a session`。
顺带发现 `skyward-mtp.rc:22` 的 `rm .../f2` 报
`No such file or directory`（清理顺序瑕疵，无害）。

#### 另一个独立 bug：logd 每 5 秒崩溃，`adb logcat` 不可用

```
init: Unable to set property 'ro.logd.kernel' ... Read-only property was already set
logd: failed to set forground cgroup: Permission denied
init: Service 'logd' (pid N) received signal 6
```

从 50 s 到 255 s 共 **30 次**，`init.svc.logd=restarting`。它每 5 秒
刷 4~5 行 dmesg，加上充电日志每 10 秒一次，**约 260 秒就把内核环形
缓冲区覆盖一轮** —— 第二份 log 里 16 s 的 init action 已经不见了。
这也是前几轮抓 log 时早期证据反复丢失的原因。镜像里已有
`system/etc/task_profiles.json`(1709 B) 和 `cgroups.json`，打包器
`main.rs:135-139` 也有补 profile 的逻辑，但 `ro.logd.kernel` 冲突与
cgroup 权限问题仍需单独处理。

#### 待确认的修复 / 规避清单（本轮未执行）

| | 做法 | 代价 |
|---|---|---|
| **A 立即规避** | Windows 设备管理器禁用 WPD「Xiaomi Pad 7S Pro 12.5」，或关 USB 选择性暂停；抓 log 时不打开资源管理器里的平板盘符；命令统一 `adb -s <设备序列号> ...` | 零风险、不刷机，但 recovery 里不能用 MTP 传文件（`adb push/pull` 可替代） |
| **B 配置层** | 让打包器可选跳过 `rd.add(MTP_RC)`，USB 只跑 `adb` 单一 function | 需重打包，Windows 不再用 WPD 枚举，adb 最稳 |
| **C 合 PR #47** | `SUSPEND/RESUME` 状态机、`ECONNRESET`、`FILE_CHUNK`、`PORT_SWITCH` 15 s | 需重编译，正面解决 WPD 挂起后的 MTP 恢复 |
| **D 修 logd** | 处理 `ro.logd.kernel` 只读冲突与 cgroup 权限 | 需重编译；日志不再刷屏，`adb logcat` 恢复 |
| **E 固化 90Hz** | 把 `devices/violin.toml` 的 `refresh` 从 60 改为 90 | 只改配置重新打包 |

建议先用 **A** 稳住抓 log 通道，下一轮把 **B+C+D+E** 合成一个新镜像
一次性验证。本轮**没有改源码、没有改主配置、没有刷机、没有写入任何
设备分区**，也没有对设备执行重启。

### 9.20 触摸间歇失灵：PR #30 的每秒无条件重写是元凶（2026-11-13 21:19）

用户反馈：**按解密密码经常按不动**；**滑动重启进度条、按住圆点右划时
经常断开触摸**。两者都是「持续按压 >1 秒的输入序列被截断」。
log 归档 `logs/20261113-2119/kernel.log`（9391 行）。

#### 因果链

`keep_touch_told()`（PR #30 等价补丁）每秒无条件
`fs::write(enable_touch_raw, "1")`。在 violin 的 NT36536 + xiaomi_touch
HAL 组合上，每次写入会带出一轮固定的驱动动作：

```
[387.811] [MI_TP_E][store_enable_touch_raw:314]: enable touch raw 1   ← 我们写
[387.977] skyward: wrote 1 to .../enable_touch_raw for the touchscreen again
[388.403] xiaomi_touch_mode: mode 1071 → nvt_set_cur_value:1071
[388.403] nvt_xm_htc_set_idle_baseline_update: set idle baseline update  ← 重设基线
[388.653] xiaomi_touch_mode: mode 1000 → nvt_set_cur_value:1000
[388.978] store_enable_touch_raw: enable touch raw 1                     ← 下一轮
```

**重设触摸基线发生在手指按住时，固件会把当前按压当成新基准，正在进行的
触点序列随之丢失** —— 正好解释长按右划被断开、短按偶尔「按不动」。

四份归档的定量对照（决定性）：

| 归档 | 版本 / 刷新率 | PR #30 | `enable_touch_raw` 写入 | `idle_baseline` | 中位间隔 | mode 分布 |
|---|---|---|---|---|---|---|
| `1901` | 1840 / 60Hz，**用户当时确认触摸正常** | 无 | **1** | 32 / 87s | **0.688 s，不规则** | 1071, 1000, **1011, 1004** |
| `2052` | 1814 / 60Hz | 有 | 111 | 112 / 127s | **1.142 s，严格** | 1071, 1000 |
| `2109` | 144Hz | 有 | 12 | 13 / 13s | 0.973 s | 1071, 1000 |
| `2119` | 90Hz | 有 | 135 | 139 / 155s | **1.154 s，严格** | 1071, 1000 |

即：合入 PR #30 后基线重置从「低频、随触摸事件触发」变成
「严格每秒一次、次数与写入一一对应」，连 HAL 下发的其它 mode（1011/1004）
都被挤掉了。

**排除项**：
- **不是 90Hz**：60Hz 的 2052（1.142 s）与 90Hz 的 2119（1.154 s）周期一致。
- **不是 input 设备重新注册**：全段 `input_register/unregister`、
  `nvt_ts_suspend/resume` 命中 **0 次**，按压序列不是被 fd 关闭截断的。
- **不是移植走样**：我们的 `keep_touch_told()` 与上游 PR #30 补丁逐字一致
  （仅注释和 `kmsg` 措辞不同）。是这个每秒无条件写在 violin 上有副作用，
  上游针对的机型驱动不是 NT36536。

#### 修复方案（待确认，本轮未改代码）

节点已只读验证：`cat enable_touch_raw` 稳定返回 `1`；同目录
`cat touch_finger_status` 返回 `0`（无手指）。因此两道保护都可行：

1. **读回比较（主）**：定时线程写之前先读节点，`trim()` 后已等于目标值
   就跳过。正常运行时**一次都不写**，PR #30 要防的「驱动 resume 把值清 0」
   仍会在下一轮被补写。改动只在 `keep_touch_told()` 内。
2. **有手指时推迟（保险）**：给 `[touch]` 加可选 `finger_status` 路径，
   写之前读它，非 `0` 就等下一轮。即使读回比较失效，也不会打断按压/
   滑动。其它机型不配该字段时行为退化为仅第 1 条。
3. **备选（最保守）**：直接删掉 `keep_touch_told()`，回到 1901 的行为。
   依据是 P0 修复后 `turn_off()` 失败留在 `Resting`，并没有真正进到
   `xiaomi_touch_resume_work` 那条息屏路径；代价是万一真息屏唤醒，
   触摸设置可能丢失。

推荐 **1 + 2** 一起做。刷入后的验证判据：
`kernel.log` 里 `store_enable_touch_raw` 应从每秒 1 次降到**开机 1 次**，
`idle_baseline` 回到低频不规则；然后实测长按滑动重启进度条与连按密码。

触摸问题优先级高于 9.19 的 B+C+D+E；可考虑与 **E（固化 90Hz）**、
**D（修 logd）** 合进同一轮构建，避免多次刷机。本轮**没有改源码、
没有改主配置、没有刷机、没有写入任何设备分区**。
