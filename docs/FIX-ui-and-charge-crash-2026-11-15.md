# violin 4.0.5 / Skyward v0.3.1 —— 充电崩溃修复与 UI 整理

日期：2026-11-15 20:09
镜像：`out/skyward-violin-OS4.0.5-20261115-2009.img`
SHA-256：`2ff6b63100d608069633ef38e6e51025553a0cfd26b071884049e9daf7eef93b`
大小：104,857,600 字节（ramdisk 49,167,337），生成于 20:09:16
日志：`logs/20261115-1956/`（WLAN 已连接）、`logs/20261115-2002/`（插拔充电器崩溃）

本轮**没有刷机、没有写入任何设备分区**；只对本地源码与打包产物做了修改。

## 1. 好消息：WLAN 已完全打通

`logs/20261115-1956/kernel.log` 里 `wpa_supplicant` 持续上报：

```
mtk_cfg80211_get_station: link speed=3000/3000, rssi=-59,
BSSID:[9c:9d:**:**:**:5e], TxOK=71, RxOK=484
```

说明开机 pre-cal（补 5 个 BT 内核模块）+ 无条件上电写入（`/dev/wmtWifi`）两处修复都生效，
WLAN 能开启、能关联、有实际数据收发。此前 11-15 的 `FIX-triage` 结论得到真机确认。

## 2. 插拔充电器后黑屏 / 掉 adb / data 未解密 / 触摸与 WLAN 全失效

### 时间线（`logs/20261115-2002/kernel.log`）

| 时间 | 事件 |
|---|---|
| 649.285 | `skyward: a charger or a computer powers the phone` |
| **649.347** | **`init: Service 'recovery' (pid 566) received signal 6`**（SIGABRT，进程被杀） |
| 649.454 | 新进程 `violin 0.3.1 recovery up` |
| **649.992** | **`recovery failed: set the panel mode: atomic commit: Permission denied (os error 13)`** |
| 649.993 | `dpu_lastclose: shutdown dpu drm device` |
| 654.353 | init 再次启动 recovery（约 5 秒后） |
| 655.877 | `xr_usb_composite_disconnect` → Windows 侧 adb 断开 |

用户看到的「黑屏、adb 断、data 未解密、不能触摸、WLAN 关闭」全部由**进程连锁重启**解释：
新进程内存里没有解密密钥、输入设备与 WLAN worker 都要重新接管，USB 也随之重枚举。

### 根因 A：充电层复用了页面尺寸的 dither（SIGABRT）

上一轮把充电动画从 `tablet.detail`（右栏，约 1576 宽）提到 `tablet.whole`（整屏，2136 宽），
但 `compose_charging()` 仍用 `self.dither.get_or_insert_with(...)` —— 这个 dither 是
`App` 里给**页面**按 `self.w/self.h`（detail 尺寸）创建的，被 About hero 与页面共用。
在更宽的 `whole` 上调用 `charge::draw()` → `glow::paint_band()` 按整屏宽度索引 dither →
**越界 panic → SIGABRT**。另外 `info.battery.as_ref().expect(...)` 与
`charge::Art::new(...).expect(...)` 在插拔瞬间也可能 panic。

修复（`crates/skyward-ui/src/app/{mod,draw}.rs`）：

- 新增 `App::overlay_dither: Option<(u32,u32,glow::Dither)>`，**与页面 dither 彻底分离**，
  按实际覆盖画布尺寸创建；尺寸变化时同时重建 `charge_art` 与 `charge_canvas`。
- `info.battery` 为 `None` 时直接跳过充电层（不再 `expect`）。
- `charge::Art::new` 失败时记日志并保留页面（不再 `expect`）。
- `charge_canvas` 加尺寸检查，避免跨尺寸复用导致另一处越界。

### 根因 B：进程刚退出时 DRM master 未释放（EACCES 连锁重启）

旧进程 abort 后 `dpu_lastclose` 需要时间，新进程 `KmsSurface::open()` 的首次
`ALLOW_MODESET` commit 被驱动以 `EACCES(13)` 拒绝，错误冒泡到 `main.rs` 的
`recovery failed` → 再退出一次，5 秒后才由 init 拉起。

修复（`crates/skyward/src/ui.rs`）：`KmsSurface::open()` 外层加最多 8 次、每次 250 ms 的
**有限重试**，只对 `PermissionDenied / WouldBlock / InvalidInput` 这类瞬时错误重试，
并写一条 kmsg。其它机型与真正的配置错误不受影响。

## 3. 四项 UI 整理

| 项 | 改动 |
|---|---|
| **无马达不该有振动设置** | `config.Device` 新增 `supports_vibration`（默认 `true`，其它机型不变）；`violin.toml` 设 `false`。`State.has_vibrator` + `vibrates()` 统一所有 `Effect::Haptic` 判定；`pages::settings()` 在无马达时**整段跳过**「振动 / 触摸振动 / 振动强度」，`通用` 段直接接在亮度滑块下。 |
| **About 署名** | `pages.rs` 的 `AUTHOR` 由 `© 2026 Kirk Lin` 改为 `© 2026 Kirk Lin · @爱看小说的书虫`，紧跟在版本号那一行下面。 |
| **主页两个 Skyward** | 分栏模式（`columns > 0`）下把首页 `Frame.title` 置空、`header = false`，并把 morph 动画里硬编码的 `HOME_TITLE` 改成 `frame.title`；左侧常驻导航列的 wordmark 保留，右侧不再重复。非分栏机型行为不变。 |
| **状态栏只在右半边** | 状态栏原本画在 `compose_page()`（`detail` 里）。现在移到 `compose_overlays()`，画在**旋转前的整屏逻辑画布 `whole`** 上；`widgets::status_bar()` 的右端定位由 `m.w` 改为 `c.w`，因此时钟贴整屏最左、电池/网络图标贴整屏最右。充电层与息屏帷幕仍在其后叠加，顺序不变。 |

`State.tablet` 由 `cfg.display.columns > 0` 派生，不需要新配置项。

## 4. 蓝牙 UI：本轮没有做，原因如实说明

用户选的 **A 方案**是「只补 BT 内核模块，让 MT6639 的共享预校准通过」，目标是修 WLAN，
**不含蓝牙功能**。当前基线里 `skyward-ui` / `skyward` / `skyward-pack` 全库无
`bluetooth / bluez / bluealsa` 引用，只有一个未接线的 `config.bluetooth` 开关，所以界面里
没有蓝牙页是预期结果。

要做出真正可用的蓝牙 UI，还缺：

- `skyward-bluetooth` crate（依赖打过 Android 补丁的 `bluer` / `dbus` / `tokio`）、
  蓝牙页面与首页入口、主程序 worker、打包器 `--bluetooth` 与 ramdisk 布局；
- BlueZ / D-Bus / BlueALSA 运行时与 `libdbus-1.so` 等动态库。ARM64 二进制已从
  `Lw5959/dijun_skyward` 的 Release 提取到 `.references/bluetooth-runtime/`
  （13 个文件 + 28 份许可证），但 **Windows 无法交叉编译这套栈**：
  `tools/build-bluetooth.py` 首行即写死 macOS/Linux，且本机缺
  `make / pkg-config / gdbus-codegen / autoconf / libtool / Ninja`。

建议把蓝牙作为**单独一轮**处理；本轮先把可用的 UI 与稳定性交付。

## 5. 构建与自检

| 项 | 结果 |
|---|---|
| 宿主 `skyward-pack` | RC=0（rustc 1.100.0） |
| Android `skyward` | RC=0，8,812,392 字节 |
| 配置校验 `stock --config` | RC=0，version / package / recovery 哈希一致 |
| 镜像内配置 | `supports_vibration=false`、`rotate=270`、`columns=560`、`refresh=90`、`reversed=["y"]`、5 个 BT 模块仍在 |
| 打包 | RC=0，104,857,600 字节 |
| `skyward/system-init` | 缺席（无双系统） |
| `logd.rc` | `priority -2` + `rlimit nice 22 22` |
| `conninfra.cfg` | `co_clock_flag=1` |
| 新代码标记 | `@爱看小说的书虫`、`keeping the page`、`the panel refused the mode` 均在二进制内 |
| ramdisk 往返 | `repack` 后 1049 条条目逐条一致 |
| 二进制一致性 | 镜像内 `system/bin/recovery` 与本地产物同为 8,812,392 字节 |

stock 底稿已改用 `D:\DNA_WIN-230828\DNA_violin-ota_full-OS4.0.5.0.XOTCNXM-user-17.0-7f96a43945\001_DNA\recovery.img`，
SHA-256 `376631cf…30293` 与配置记录一致。

## 6. 刷入后请重点验证

1. **反复插拔充电器**：不再黑屏、不掉 adb、不重新解密、触摸与 WLAN 保持。
   log 里应不再出现 `received signal 6` / `recovery failed`；
   若出现 `the panel refused the mode; trying again`，属于预期的重试，不该导致重启。
2. **充电动画**铺满整屏（含左侧导航列），息屏两栏同步变暗。
3. **状态栏**：时钟在最左、电池在最右，横跨整屏而不是只在右半。
4. **设置页**不再有「振动 / 振动强度」；音量键与滑动也不再请求振动反馈。
5. **About 页**第二行出现 `@爱看小说的书虫`。
6. **主页**只有左侧一个 Skyward 标题。
7. 90Hz、触摸坐标、WLAN 连接没有回归。

回退：`out/skyward-violin-OS4.0.5-20261115-1940.img`（`286332da…5094`），或原厂
`001_DNA/recovery.img`。只刷 `recovery_b`，`fastboot reboot recovery` 本机实测可用。


## 7. 真机验证（20:17，镜像 2ff6b631…93b1）

用户确认：**插拔充电器正常、充电动画正常**（本轮两项崩溃/覆盖层修复生效）。
本轮只读日志 `logs/20261115-2017/`（kernel.log 6298 行 / props / logcat）：

- `underflow` 0、`recovery failed` 0、`received signal 6` 0、
  `the network on wlan0 reaches the internet` 1 次 —— 稳定且 WLAN 仍连着。
- `adb shell cat /sys/class/drm/card0-DSI-1/modes` 列出 7 档：
  `3200x2136x120 / 144 / 90 / 60 / 50 / 48 / 30`，确认 90 档存在。
- 本次 dmesg 环形缓冲只保留 40.9s→126.4s，开机 15s 的
  `new refresh rate is 90(hz)` 已被 WLAN/充电日志覆盖；**随后用户重启后抓到完整开机段，
  该行已直接确认，见第 8 节**。
- **90Hz 的确定性证据**（不依赖肉眼）：
  1. `surface.rs::open(path, Some(90))` 用 `find(|m| m.vrefresh() == 90)`，
     找不到会 `anyhow!("the panel offers no 90 Hz mode...")` 让 recovery 起不来；
     当前 UI 正常显示 ⇒ 必然已选中并提交 90Hz mode。
  2. `let preferred = if refresh.is_some() { &mode } else { preferred }` 把上电、
     `to_fastest()`、熄屏唤醒全部统一到 90Hz，不会瞬时跳 144Hz。
  3. 源码与镜像内配置都是 `refresh = 90`。
  4. 历史真机日志 `logs/20261113-2114/kernel.log:15.865` 与
     `logs/20261114-1028/kernel.log:14.960` 均有
     `[DPU-I] dpu_crtc_print_commit_info: new refresh rate is 90(hz)`，且 0 underflow；
     144Hz 那次（`20261113-2109`）是 903 次 underflow 花屏。
- 自查（无需刷机）：息屏再亮屏后 `adb shell dmesg | grep -i "new refresh rate"`，
  应出现 `new refresh rate is 90(hz) old 90(hz)`；或重启 recovery 后 20 秒内抓开机段。
  `/system/bin/recovery kms` 在这台 EDK2 设备上返回 RC=255、无输出，不可依赖。
- 可选改进：把 `ui.rs:1297 eprintln!("display {}", surface.describe())` 同时写入
  kmsg，开机日志即永久保留 `3200x2136 90 Hz, crtc ...`，不必抢在缓冲覆盖前抓。
  需重新打包刷入，本轮未做。
- 设备最后状态：adb 瞬时掉线（Windows WPD 复合设备重枚举，9.19 已定位），
  未重试、未重启、未写任何分区。


## 8. 重启后的完整开机日志：adb 掉线的确切原因与 90Hz 直接证据

日志 `logs/20261115-2020/kernel.log`（8661 行，从 `0.000000` 起，完整开机段）。
用户重启 recovery 后 adb 又断，`adb kill-server && adb devices` 后**立刻恢复**
（transport_id 34 → 1），设备侧 `sys.usb.state=mtp,adb`、`init.svc.adbd=running`
始终正常 —— 说明设备并没有真正掉线。

### 90Hz 终于有了一手证据

```
[   15.281966] [DPU-I]: [CORE]: dpu_crtc_print_commit_info:
               [CRTC:63][frame_no:0] new refresh rate is 90(hz)
```

全日志 `underflow` 0 次。与第 7 节的代码逻辑、配置、sysfs 档位一致，
不必靠肉眼判断。

### adb「进来一会就断」= 开机 15.6 秒设计内的一次 USB 重配

| 时间 | 事件 |
|---|---|
| 9.460 | `skyward: violin 0.3.1 recovery up` |
| 9.520 | `sys.usb.config=adb` → 启动 adbd (pid 346) |
| **15.600** | `sys.usb.config=none`（`init.rc:189`、`skyward-mtp.rc:21`） |
| **15.604** | `xr_usb_composite_disconnect` + unbind / replace gadget driver ops（**唯一一次**；grep 计成 3 行是同一事件的 enter/exit） |
| 15.605 | adbd 346 被 SIGKILL |
| 15.681 | `sys.usb.config=mtp,adb` → 重启 adbd (pid 457) |
| 16.058 / 16.102 | `mtp: the computer configured the function` / `opened a session` |
| 16.2 → 55.9 | **零断开、零 MTP 错误、零端口切换** |

`skyward-mtp.rc` 为了启用 MTP 文件传输，把 USB 从 `adb` 经 `none` 重绑成
`mtp,adb`。这次 gadget 重绑让 Windows 的复合设备（MTP `MI_00` + ADB `MI_01`）
重新枚举；WPD 一侧处理重枚举时没把 ADB interface 带回来，于是 adb 一直显示离线，
直到主机侧强制重新扫描。这与 9.19 节「Windows WPD 复合设备重枚举」的结论一致，
现在有了完整开机时间线。

**排除**：`recovery failed` 0、`received signal 6` 0（不是崩溃）；
`file transfer stops` 0、`port kept file transfer` 0（PR #47 修的 MTP 端口切换
路径这次根本没触发）；与充电、WLAN、刷新率无关。

附带一条无关但值得记的警告：`bcdVersion must be 0x0100, stored in Little
Endian order ... accepting 0x0001` —— FunctionFS 描述符字节序警告，
可能让 Windows 枚举更敏感，但不是本次断线主因。

### 刷机掉线会不会黑砖：不会

- 刷 recovery 走的是 **bootloader / fastboot 的 USB 通道**，与 recovery 里
  的 adbd / MTP 是不同阶段、不同 USB function；上面的 15.6s 重配根本不在刷机路径上。
- `fastboot flash recovery_b` 只写 **recovery 分区**（100 MiB），不碰
  `bootloader* / abl / xbl / preloader / boot / init_boot / vendor_boot / vbmeta* / 分区表`。
  bootloader 完好 ⇒ 永远能进 fastboot ⇒ 随时可重刷。
- 即使刷到一半断线，最坏是 recovery 分区内容不完整、recovery 起不来；
  **正常系统照常能开机**（系统分区完全没动），因为 bootloader 从当前槽的 boot 启动。
- 恢复只需：重插线 → `fastboot devices` → `fastboot flash recovery_b <镜像>` →
  `fastboot reboot recovery`。原厂底稿 `001_DNA/recovery.img`
  （SHA `376631cf…30293`）与 skyward 镜像都在本机；`recovery_a` 保留原厂版作第二重保险。
- 这台设备**没有 EDL**，所以更要坚持「只刷 `recovery_b`」：不要 `--slot all`、
  不要 `flashall` / `update` / `erase` / `format` / `flashing lock|unlock`。
  只要不碰 bootloader 类分区与分区表，就没有硬砖通道。

### 让 adb 不再掉的三个方案

- **B（推荐，直接消除触发点）**：打包器可选跳过 `MTP_RC`，USB 只跑单一 `adb`
  function —— 开机不再有 `none ↔ mtp,adb` 重绑，复合设备也不再重枚举。
  代价：recovery 里没有 MTP 盘符，用 `adb push/pull` 传文件。需改打包器 + 重新打包。
- **C**：合 PR #47（SUSPEND/RESUME 状态机、ECONNRESET、PORT_SWITCH 5→15s）。
  本次日志显示 MTP 会话正常、端口切换未触发，故优先级低于 B。
- **零改动规避**：Windows 设备管理器禁用 WPD「Xiaomi Pad 7S Pro 12.5」、
  关闭 USB 选择性暂停、抓 log 时不打开资源管理器盘符、命令带
  `-s <设备序列号>`、掉线时 `adb kill-server && adb devices`、
  刷机用后置 USB 口与原装线。

本轮纯只读诊断：未改代码、未打包、未刷机、未写任何设备分区、未重启设备
（`adb kill-server` 只重启了 PC 上的 adb server）。
