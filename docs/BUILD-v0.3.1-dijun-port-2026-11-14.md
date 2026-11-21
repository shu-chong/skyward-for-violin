# 构建记录：v0.3.1 基线 + dijun 修复 + violin 重打（2026-11-14）

本轮在官方 `v0.3.1` 干净基线上合并同 XRING O1 的 `Lw5959/dijun_skyward` 修复、
重打 violin 补丁，并按 dijun 的 MT6639 启动链启用 WLAN，最后构建 recovery 镜像。
**未连设备、未刷机、未写任何设备分区。**

## 1. 交付镜像

| | |
|---|---|
| 路径 | `out/skyward-violin-OS4.0.5-20261114-0954.img` |
| 大小 | 104,857,600 字节（ramdisk 49,161,126） |
| **SHA-256** | `cb08d374ed127f6af78a97440d136875b6bfc180fd5feec440bf1afdc57e89b1` |
| 生成 | 2026-11-14 09:54:19 |
| Android 主程序 | `8,804,664` 字节，交叉编译 RC=0 |
| 宿主打包器 | RC=0（rustc 1.100.0） |

## 2. 修改清单

### 2.1 Windows 宿主构建链（0.3.1 官方只支持 Unix）
- `skyward-bootimg/src/ramdisk.rs`：`#[cfg(unix)]` 隔离 `unpack` / `c_path` / `set_time` / imports / tests，加 `cfg(not(unix))` 桩。
- `skyward-block/src/mount.rs`：0.3.1 新增的 `space()` / `statvfs()` 加 `#[cfg(unix)]` + 非 Unix 桩，相关 imports 与测试同样隔离。
- `skyward-block/src/erase.rs`：`page_size()` 的 `sysconf` 加隔离 + 4096 桩。
- `skyward-sys/src/lib.rs`：`bcb/clock/control/cpu/ueventd/usb/worker` 模块与 `since_boot()` 加 `#[cfg(unix)]`。依赖链是 `pack → programs → omapi → binder → sys`，`binder` 在宿主只需 `prop`（已有桩）。
- `skyward-config/src/lib.rs`：5 处 `Path::is_absolute()` → `starts_with('/')`（Windows 会把 `/system/...` 判为相对路径）。
- `skyward-pack/src/main.rs`：`--system-init` 改为可选，本构建不传（镜像无 `skyward/system-init`）。

### 2.2 配置层（`skyward-config`）
加回/新增字段：`DeviceConfig.bluetooth`、`Display.refresh`、`Touch.reversed`、
`Touch.xiaomi_double_tap`、`Service.optional`、`Wlan.off_writes`。
同步给 `skyward-programs` 里 3 处 `Service` 字面量补 `optional: false`。
0.3.1 的 config 不 `deny_unknown_fields`，所以这些字段缺失时旧 `violin.toml` 会被**静默忽略**——
这是升级后最容易踩的坑，已全部补回。

### 2.3 dijun P0 修复 + violin 补丁
| 文件 | 改动 |
|---|---|
| `skyward-ui/src/surface.rs` | `open(path, refresh)` 按配置选 90Hz，并把 `preferred` 重绑到该 mode（上电/唤醒都用它）；`turn_off()` 在 `lp.is_none()` 时完整解绑所有 plane 的 `FB_ID`/`CRTC_ID`、connector `CRTC_ID`、清 `MODE_ID`，`present()` 成对恢复 —— **替换 0.2.0 的 `Resting` 降级** |
| `skyward/src/input.rs` | `swapped` + `reversed=["y"]` 坐标映射；**不引入 PR #30 的每秒定时重写**（0.3.1 官方没有，dijun 也没有）→ 长按滑动不再被基线重置打断 |
| `skyward/src/ui.rs` | `normal_max()` 非高通回退；传 `refresh`/`reversed`；`dual_boot = false`；移除 `Target::Edl` 映射 |
| `skyward-ui/src/app/reboot.rs` / `pages.rs` / `search.rs` | 重启页去掉高通专用 EDL 项（TARGETS 6→5，文案表 6→5，搜索关键词） |
| `skyward-block/src/fstab.rs` | 动态扫描 `/vendor/etc`、`/odm/etc` 下所有 `fstab.*`（violin 是 `fstab.O1_asic`）；移植 dijun `preserve_o1_data()`：去 `check/formattable/resize`、persist/nvdata 改 `ro,noload`、移除 `rescue→/cache` |
| `skyward/src/services.rs` | `optional` 服务未就绪只记日志、不终止 recovery |
| `skyward/src/main.rs` | sharedsecret 协商失败只记日志 |
| `skyward/src/wlan.rs` | 关闭 WLAN 时执行 `off_writes`（`wmtWifi=0`，交还电源控制） |
| `skyward-pack/src/main.rs` | 打包期把 stock `system/etc/init/logd.rc` 的 `priority 10` 改成 `priority -2` + `rlimit nice 22 22`（修 logd 每 5 秒 SIGABRT）；写入 `system/etc/firmware/conninfra.cfg = co_clock_flag=1`（MT6639 共享预校准） |

### 2.4 `devices/violin.toml`
- `refresh = 90`（真机验证 0 underflow）。
- `[modules].load` 开机一次连续加载 `rfkill → conninfra → wmt_chrdev_wifi_connac3 → wlan_drv_gen4m_eap_6639` + 4 个触摸模块。
- 服务链按 dijun：`conninfra_loader`（oneshot）→ `setprop vendor.mtk.nvram.ready 1`（oneshot）→
  `uart_launcher -f 3 -p /dev/ttyXR0` → `wlan_assistant`（optional）→ 触摸/解密/启动控制服务。
  全部跑 root（recovery 不执行系统 init 的 chown）。
- `[wlan]`：`modules = []`，只保留 `wmtWifi=1` / `off_writes wmtWifi=0`、`wlan0`、wpa_supplicant、
  `ISupplicant/default`、miui 204 probe。
- `[partitions] mounted = ["persist", "nvdata"]`。
- 保留 4.0.5 stock 哈希、backup、wipe、update；无 haptics（无马达）、无双系统。

## 3. 自检结果（全部通过）

- `skyward-pack stock --config devices/violin.toml` RC=0。
- 镜像内 `system/bin/recovery` = 本地产物 `8,804,664` 字节。
- 镜像内配置：`refresh = 90`、`reversed = ["y"]`、`mounted = ["persist","nvdata"]`、`[wlan]` 与 `off_writes` 齐全。
- `system/etc/init/logd.rc` 含 `priority -2` + `rlimit nice 22 22`。
- `system/etc/firmware/conninfra.cfg` = `co_clock_flag=1`。
- `skyward/system-init` **缺席**（无双系统）。
- `repack` 往返后 **1049 条 ramdisk 条目逐条一致**（共 1050 行含表头）。

## 4. 未完成项（如实说明）

### 4.1 蓝牙：运行时已备好，功能未合入

用户要求蓝牙也按 dijun 方案。**本轮镜像不含蓝牙功能**，原因不是遗漏，而是本机硬约束：

- dijun 的 `tools/build-bluetooth.py` 首行写死 `host = {"Darwin", "Linux"}[platform.system()]`，
  Windows 会直接 `KeyError`；它要交叉编译 BlueZ 5.88 / glib / dbus / expat / BlueALSA /
  libsamplerate，需要 `make`、`pkg-config`、`gdbus-codegen`、`autoconf`、`libtool` —— 本机全部缺失。
- **已绕过的部分**：从 dijun Release (`v0.2.1` 的 49 MB zip / 100 MiB recovery 镜像) 里
  提取出**完整的 ARM64 蓝牙运行时**：13 个文件（`bluetoothd`、`dbus-daemon`、`bluealsad`、
  `bluealsa-aplay`、`skyward-bt-bridge`、`skyward-audio-route`、3 个 conf、2 个 sources.json）
  + 28 份许可证，已存 `.references/bluetooth-runtime/`。
- **仍缺的部分**：`skyward-bluetooth` crate（依赖 `bluer` / `dbus` / `tokio`，
  dijun 用的是打过 Android 补丁的 `vendor/bluer`）、`skyward-ui/src/pages/bluetooth.rs`
  页面、首页入口、主程序 worker、打包器 `--bluetooth` 参数。0.3.1 把 `app.rs` 拆成了
  `app/` 20 个模块、安装器改用 `Worker`，这些代码不能原样套用，需单独一轮移植并配好
  `libdbus-1.so` 等动态库（NDK 里没有，也要从运行时包里补）。
- 另外 dijun 的音频路由是 15S Pro 的 CS35L43 / O1 声卡专用，violin 是否同款未验证。

下一轮做蓝牙的前置条件：把 `vendor/bluer` 与 `skyward-bluetooth` 移植到 0.3.1 API、
补 UI 页面与 worker、给打包器加 `--bluetooth`、确认动态库齐备。

### 4.2 双击亮屏：未合入
dijun 的 `touch.rs`（`/dev/xiaomi-touch` ioctl，`SELECT_TOUCH_ID(0)` + mode 14 + 回读校验）
和 `KEY_WAKEUP → Event::Wake` 需要改 0.3.1 的 `app/` 状态机（`wake_gesture`、设置页开关）。
本轮未做；而且 NT36536 是否支持 mode 14 尚未验证，建议先只读 ioctl 探测再接 UI。

### 4.3 其它
- `install/run.rs` / `systems/install.rs` 的「拒绝第二系统包」桩未重打，但
  `dual_boot = false` + 镜像无 `system-init`，第二系统仍不可达、不可启动。
- `module_graph()`（O1 版本目录 `modules.dep`）未移植：本配置用到的模块都在平铺目录，不阻塞。
- dijun 的「已存 WLAN 网络导入 / 重连提速」体验增强未移植；0.3.1 官方尚无这些功能。
- 打包器的 stock 整镜像 SHA 校验 / AVB rollback 继承未合入。

## 5. 刷入后验证要点

1. 开机是否 90Hz 无花屏；息屏/亮屏是否不再出现 `dpu_power_check ... ret -22`
   和 `recovery failed`（完整解绑方案是否生效）。
2. 长按滑动重启进度条、连按解密密码是否还断触（应已无每秒基线重置）。
3. `logcat` 是否可用、`dmesg` 是否不再每 5 秒被 logd 崩溃刷屏。
4. WLAN 页面能否开启：看 `pre_cal_flow`、`Begin pre-cal`、`wlan0` 是否出现，
   以及 `wlan_assistant` / `uart_launcher` / `conninfra_loader` 是否按顺序就绪。
5. data 能否解密（服务全部可选、sharedsecret 失败不终止）。
