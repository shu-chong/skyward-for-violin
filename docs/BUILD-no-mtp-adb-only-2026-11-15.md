# violin 4.0.5 / Skyward v0.3.1 —— 只跑 adb 的 USB 版本（方案 B）

日期：2026-11-15 20:36
镜像：`out/skyward-violin-OS4.0.5-20261115-2036.img`
SHA-256：`fafe86d90948c6b9c9fb9db70014a5f5347308d2a9ca607151d88f8dea215da5`
大小：104,857,600 字节（ramdisk 49,167,023），生成于 20:36:35

本轮**没有刷机、没有写入任何设备分区**。

## 1. 要解决的问题

`logs/20261115-2020/kernel.log`（完整开机段）显示 adb 在开机约 15.6 秒掉线：

| 时间 | 事件 |
|---|---|
| 9.520 | `sys.usb.config=adb` → adbd pid 346 |
| 15.600 | `sys.usb.config=none`（`skyward-mtp.rc:21` / `init.rc:189`） |
| 15.604 | `xr_usb_composite_disconnect` + gadget unbind / replace driver ops |
| 15.605 | adbd 346 被 SIGKILL |
| 15.681 | `sys.usb.config=mtp,adb` → adbd pid 457 |
| 16.058 | MTP 会话建立 |

触发者是运行时 `Transfer::start()` 里的 `skyward_sys::usb::set_mtp(true)`：
它为启用 MTP 文件传输改了 `sys.usb.config`，init 于是把整个 USB gadget
经 `none` 重绑成 `mtp,adb`。Windows 的复合设备（MTP `MI_00` + ADB `MI_01`）
重新枚举时把 ADB interface 弄丢，设备侧其实一直是 `sys.usb.state=mtp,adb`、
`init.svc.adbd=running`。`adb kill-server && adb devices` 立刻恢复可证。

## 2. 做法：一个 `mtp` 开关贯穿配置、打包器、运行时与 UI

`DeviceConfig.mtp`（`#[serde(default = "default_true")]`，**默认 true，其它机型行为不变**），
`violin.toml` 顶层设 `mtp = false`：

| 位置 | 改动 |
|---|---|
| `skyward-config` | 新增顶层 `mtp: bool`。 |
| `skyward-pack` | `cfg.mtp` 为 false 时**不写 `system/etc/init/skyward-mtp.rc`**；并在 `prop.default` 尾部追加 `persist.sys.usb.config=adb`（后行覆盖前行）。 |
| `skyward/src/ui.rs` | `app.state.mtp = cfg.mtp`、`file_transfer = false`；`Pages::open` 的 `if cfg.mtp && app.state.file_transfer` 保证**不启动 MTP Service、不调 `set_mtp(true)`**；`file_transfer()` 开头直接拒绝并复位状态。 |
| `skyward-ui` | `State.mtp`（默认 true）；`pages::settings()` 在无 MTP 的构建里跳过整个「USB / 文件传输」段，调试段紧接通用段。 |
| `skyward-sys/usb.rs` | **`switch()` 加幂等保护**：`sys.usb.state` 已等于目标配置就直接返回，不再走 `none → 目标`。因为 `resting()` 在 `MTP=false` 时返回 `adb`，息屏/唤醒/退出 fastboot 等恢复端口的路径若不幂等，仍会重绑 gadget。 |

另外 `usb::resting()` 在 `MTP=false` 时本来就返回 `adb`，与新的默认一致。

**代价**：这版 recovery 不再有 MTP 盘符，传文件用 `adb push/pull`。
收益是开机只有一次 `sys.usb.config=adb`、adbd 只启动一次、不再有 15.6 秒的
gadget 重绑，Windows 也不需要重新枚举复合设备。

## 3. 构建与自检

| 项 | 结果 |
|---|---|
| 宿主 `skyward-pack` | RC=0（rustc 1.100.0） |
| Android `skyward` | RC=0，8,812,792 字节 |
| 配置校验 `stock --config` | RC=0，`mtp=false` / `supports_vibration=false` / `refresh=90` |
| **镜像内 `skyward-mtp.rc`** | **缺席**（上一版 `…2009.img` 对照计数 1） |
| `prop.default` 尾部 | `ro.secure=0`、`ro.adb.secure=0`、`ro.debuggable=1`、`ro.ota.allow_downgrade=true`、`persist.sys.usb.config=adb` |
| 镜像内配置 | `mtp=false`、`supports_vibration=false`、`rotate=270`、`columns=560`、`refresh=90` |
| `skyward/system-init` | 缺席（无双系统） |
| `logd.rc` | `priority -2` + `rlimit nice 22 22` |
| `conninfra.cfg` | `co_clock_flag=1` |
| 二进制一致性 | 镜像内 `system/bin/recovery` 与本地产物同为 8,812,792 字节 |
| ramdisk 往返 | `repack` 后 1048 条条目逐条一致 |

stock 底稿：`D:\DNA_WIN-230828\DNA_violin-ota_full-OS4.0.5.0.XOTCNXM-user-17.0-7f96a43945\001_DNA\recovery.img`，
SHA-256 `376631cf…30293` 与配置记录一致。

## 4. 刷入后请重点验证

1. **开机日志**（重启后 20 秒内 `adb shell dmesg`）：
   - 不再出现 `sys.usb.config=none` / `xr_usb_composite_disconnect` /
     `Sending signal 9 to service 'adbd'`；
   - `starting service 'adbd'` 只有 **1 次**；
   - `sys.usb.state` 全程为 `adb`（`adb shell getprop sys.usb.state`）。
2. **adb 不再掉线**：连续几分钟 `adb devices -l` 稳定，transport_id 不变。
3. `new refresh rate is 90(hz)` 仍在、`underflow` 0。
4. 设置页不再有「文件传输」开关（也不再有振动项）。
5. 充电插拔、WLAN、触摸、旋转、充电动画无回归。

回退：`out/skyward-violin-OS4.0.5-20261115-2009.img`（`2ff6b631…93b1`）或原厂
`001_DNA/recovery.img`。只刷 `recovery_b`，`fastboot reboot recovery` 本机实测可用。
