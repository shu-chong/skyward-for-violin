# violin 真机反馈四症状：诊断与修复（2026-11-15）

交付镜像：`out/skyward-violin-OS4.0.5-20261115-1940.img`
SHA-256 `286332da26c8e0d39b392d96585f696cbba91f232ed6a9d016809525095fd094`
104,857,600 字节（ramdisk 49,166,555），生成于 19:40:27。

诊断日志：`logs/20261115-1926/`（8301 行内核日志，recovery 内以 root 抓取）。
本轮**没有刷机、没有写入任何设备分区**；只在本机编译、打包。

---

## 用户反馈与结论

| 症状 | 结论 | 处理 |
|---|---|---|
| 画面方向颠倒 | `rotate = 90` 在真机是倒的 | 改 `270`，配置即可，不动代码 |
| 充电动画只在右边显示 | **上轮实现缺陷** | 把整屏覆盖层提到逻辑整屏 `whole` 画布 |
| 息屏时右侧先黑、左侧导航列后黑 | 同上，curtain 只作用于 detail | 同上 |
| WLAN 打不开 | **`bring_up()` 上电短路** | 把 `cfg.writes` 移出 `if !net.exists()` |
| adb 只能充电、不识别 | **设备侧完全正常**，Windows WPD 复合设备重枚举 | 不改 recovery，给规避方法 |

## 1. 先说 WLAN：补 BT 模块这一环已经生效

`logs/20261115-1926/kernel.log` 证据：

```
10.414  skyward: loaded .../bluetooth.ko / hci_uart.ko / hidp.ko / rfcomm.ko / btmtk_uart_unify.ko
14.926  connv3_core_subsys_ops_reg [pre_cal] type=[WIFI] ... bt=[NULL][NULL] wf=[有值]
15.008  connv3_core_subsys_ops_reg [pre_cal] type=[BT]  → bt=[有值][有值] wf=[有值]
15.008  [pre_cal] trigger pre-cal BT/WF are registered
15.008  BT&WIFI is off, schedule pre-cal from status=[0] to new status[2]
16.055  [pre_cal] >>>>>>> pre on DONE!!
16.105  btmtk_pre_cal_pwr_on_cb start
18.677  BT turn on ok!  / sub_bt_thrd DONE / sub_wifi_thrd DONE
15.848  [wifi] wlan0 创建（nicApplyNetworkAddress WLAN0 mac）
```

对比上一版（11-14 10:28）`bt=[NULL][NULL]` + `pre_cal_flow: 0` + `wlan_func_on_by_chrdev
Timeout` —— **pre-cal 已从超时变成全流程成功**，`wlan0` 也建出来了。补 5 个 BT 内核
模块是对的，不需要 BlueZ/D-Bus/BlueALSA/蓝牙 UI。

### 但 WLAN 仍打不开的真正原因

`crates/skyward/src/wlan.rs::bring_up()`：

```rust
let net = Path::new("/sys/class/net").join(&cfg.interface);
if !net.exists() {
    ... firmware / modules ...
    for w in &cfg.writes { fs::write(&w.path, &w.value)?; }   // ← 上电写 1 在这里
    ... 等接口出现 ...
}
iface::set_up(&cfg.interface, true)
```

开机 pre-cal 已经创建了 `wlan0`，随后把芯片断电（日志 `Wi-Fi power off done!`），
接口残留但 `operstate=down`、`flags=0x1002`（无 IFF_UP）。用户点开 WLAN 时
`net.exists()` 为真 → **整个上电分支被跳过** → 没写 `1` 到 `/dev/wmtWifi` →
芯片没电 → `wpa_supplicant` 起不来。

现场核对：`/sys/class/net/wlan0` 存在、`operstate=down`、无 `wpa_supplicant` 进程、
日志里 `WIFI_write` 只有 1 次且是 `wlan_assistant` 写 NVRAM（`WR-BUF:NVRAM9...`），
**没有点开 WLAN 的第二次上电写入**。

**修复**：把 `cfg.writes` 移出 `if !net.exists()`，无条件上电；模块/固件仍只在
接口不存在时加载，等待接口出现的循环保留。MTK WMT 的 `WIFI_write 1` 幂等，
且 `off()` 的 `off_writes` 写 `0` 与之配对，重复开关不会叠加状态。

## 2. 充电动画 / 息屏黑屏：覆盖层作用域错了

上轮把 `App::compose()` 的 out 从全屏 `screen` 换成了 `tablet.detail`（右栏），
而左侧导航列是 `Tablet::draw_sidebar()` 独立绘制的。`compose()` 末尾的
`compose_charging()` 和 `out.dim(curtain)` 都只落在 `detail` 上 →
充电动画只在右栏、息屏渐暗只作用于右栏，左栏要等真正 `blanked`
（`screen.data.fill(0)`）才黑，于是出现"右边先黑、左边后黑"。

**修复**（拆成整屏覆盖层，逻辑画布在旋转之前）：

| 文件 | 改动 |
|---|---|
| `skyward-ui/src/app/draw.rs` | `compose()` 拆为 `compose_pages()`（只画页面）+ `compose_overlays()`（充电层 + curtain）；`compose_charging()` 的 theme 按 `out.w/out.h` 构造，让充电画面按整屏排版 |
| `skyward-ui/src/tablet.rs` | 新增 `whole: Canvas`（逻辑整屏，未旋转）；`paint()` 拆为 `assemble()`（导航列 + 页面合到 `whole`）和 `turn()`（`whole` 旋转到物理 `screen`） |
| `skyward/src/ui.rs` | `show()` 顺序改为 `compose_pages(detail)` → `assemble()` → `compose_overlays(whole)` → `turn(screen)` → `present` |

这样充电层与息屏渐暗都覆盖整个逻辑画布，两栏同步；息屏全黑分支
（`screen.data.fill(0)`）本来就是整屏，不变。

## 3. 旋转方向

`devices/violin.toml`：`rotate = 90 → 270`。`Tablet::place()` / `unplace()`
用同一个 `Rotation`，触摸逆变换自动跟着翻转，已验证的 `swapped +
reversed=["y"]` 映射层一行未动。

## 4. adb「只能充电」——设备侧正常，别改 recovery

日志与属性都显示设备侧 USB 完全正常：

```
sys.usb.config = mtp,adb
sys.usb.state  = mtp,adb
sys.usb.ffs.ready = 1 / sys.usb.ffs.mtp.ready = 1
init.svc.adbd  = running
15.979  init: processing action (sys.usb.config=mtp,adb ...) → starting service 'adbd'
16.360  skyward: mtp: the computer configured the function
16.403  skyward: mtp: the computer opened a session
/sys/class/android_usb/android0/state = CONFIGURED
```

而且本轮日志就是**通过 adb 以 root 抓到的**（`adb devices` → `recovery`）。
开机 15.88s 那次 `sys.usb.config=none → mtp,adb` + adbd SIGKILL/重启是设计内的
一次重配，只发生一次，之后没有断开。

这与 9.19 节结论一致：`VID_18D1&PID_4EE2` 是 MTP(WPD) + ADB 复合设备，
Windows 侧 WPD 后台轮询 / USB 选择性暂停 / 资源管理器浏览盘符时会让整个
复合设备重枚举，ADB interface 跟着瞬时消失，设备侧无感。

**规避（零风险，不用刷机）**：
1. 设备管理器禁用 WPD「Xiaomi Pad 7S Pro 12.5」；
2. 关闭 USB 选择性暂停；
3. 抓 log 时不要在资源管理器打开平板盘符，文件传输用 `adb push/pull`；
4. 命令带序列号：`adb -s <设备序列号> ...`；
5. 掉线时 `adb kill-server && adb devices` 即可，不必重启设备。

## 5. 构建与自检

| 项 | 结果 |
|---|---|
| `cargo build -p skyward-pack`（宿主） | RC=0 |
| `cargo build --target aarch64-linux-android -p skyward` | RC=0，8,811,096 字节 |
| stock 底稿 | D 盘 `001_DNA/recovery.img`，SHA `376631cf…30293` 与配置一致 |
| 打包 | RC=0，ramdisk 49,166,555 字节 |
| `repack` 往返 | 1049 条逐条一致 |
| 镜像内配置 | `refresh=90`、`rotate=270`、`columns=560` |
| BT 模块 / logd / conninfra.cfg | 均保留 |
| `skyward/system-init` | 缺席（无双系统） |

## 6. 刷入后请验证

1. 画面方向是否正（270）；若仍倒，翻回 90 或改 180 需另议；
2. 充电动画是否铺满整屏、息屏时两栏是否同步变暗；
3. **点开 WLAN 后**看日志里是否有第二次 `WIFI_write` / `WIFI_open`、
   `wpa_supplicant` 是否起来、`wlan0` 是否 `up`；
4. 触摸坐标（旋转翻到 270 后尤其要复核）；
5. adb 若再掉线，按上面 5 条规避，不是 recovery 的问题。

## 7. 回退

- 上一版：`out/skyward-violin-OS4.0.5-20261114-1140.img`（`dfa73fd5…`）
- 只刷 `recovery_b`，保留 `recovery_a` 原厂版；`fastboot reboot recovery` 本机可用。
