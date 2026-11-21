# 真机诊断：WLAN 打不开 / 平板 UI（2026-11-14 10:28）

镜像：`out/skyward-violin-OS4.0.5-20261114-0954.img`（`cb08d374…89b1`）
日志：`logs/20261114-1028/`（`kernel.log` 8826 行、`props.txt`、`mounts.txt`、`cmdline.txt`）
本轮**只读**：抓 log、读源码，未改代码、未刷机、未写设备分区。

## 0. 先说好消息：本轮 P0 修复全部生效

| 检查 | 结果 |
|---|---|
| 刷新率 | `new refresh rate is 90(hz)`，**0 次 underflow** |
| 息屏 EINVAL | 无 `dpu_power_check ... ret -22`、无 `recovery failed`、recovery 进程只启动 1 次 |
| 触摸定时重写 | `for the touchscreen again` **0 次**（PR #30 没有引入，长按断触的根因已消除） |
| logd | `Service 'logd' received signal 6` **0 次**（`priority -2 + rlimit nice 22 22` 生效），不再每 5 秒刷屏 |
| 模块 / 服务 | 全部 WLAN 模块按依赖加载，`conninfra_loader` / `setprop` / `uart_launcher` / `wlan_assistant` 都执行了 |
| NVRAM | `wlan_assistant` 成功读到校准数据：`Set NVRAM state[1]`、`WIFI_write WR-BUF:NVRAM9… length 6156` |
| 挂载 | `persist` / `nvdata` 均 `ro,norecovery`（noload），数据保全生效 |
| 解密 | `2 of the secure world agreed on the key`，`/data` f2fs 已识别 |

另有一处**与 WLAN 无关**的内核 WARNING（只出现 1 次，开机时）：
`memcpy: detected field-spanning write (size 5192) of single field "tp_frame->tp_raw" at nt36xxx.c:3146 (size 5176)`
是 NT36536 触摸驱动自身的越界告警，暂未观察到功能影响，记录备查。

## 1. WLAN 打不开：根因是「BT 回调没注册，预校准起不来」

### 时间线（内核日志）

| 时间 | 事件 |
|---|---|
| 9.596 | `connv3_core_init [pre_cal] Init g_pre_cal_mode = 4` |
| 9.755 | 模块加载完毕：`rfkill / conninfra / wmt_chrdev_wifi_connac3 / wlan_drv_gen4m_eap_6639 / cfg80211 / connfem / pcie_xring` |
| 14.345 | `skyward: ran /vendor/bin/conninfra_loader` |
| 14.349 | `skyward: ran /system/bin/setprop vendor.mtk.nvram.ready 1` |
| **14.367** | `connv3_core_subsys_ops_reg [pre_cal] type=[1] bt=[0000000000000000][0000000000000000] wf=[有值][有值]` ← **BT 回调是 NULL** |
| 14.367 | `wlanNvramUpdateOnTestMode: device not ready return` |
| 14.35 起 | `uart_launcher` **每 ~24 秒退出一次**（pid 379 / 430 / 432 / 433，exit status 0），被 supervise 反复重启，从未稳定驻留 |
| 17.847 | 用户点开 WLAN → `WIFI_write 1` → `get_pre_cal_status: pre cal status: 0` |
| **27.860** | `wlan_func_on_by_chrdev: Timeout, pre_cal_flow: 0, locked: 0` → `WMT turn on WIFI fail!` |
| 27.860 | `skyward: the WLAN did not come up: write 1 to /dev/wmtWifi: I/O error (os error 5)` |
| 43.886 / 53.897 | 第二次点开，完全相同的失败 |

### 判断

MT6639 是 **Wi-Fi / BT 共享芯片**，`g_pre_cal_mode = 4` 的预校准流程要求 BT 侧
回调也注册。日志里 `bt=[NULL][NULL]`、`pre_cal_flow: 0`、`pre cal status: 0`
说明这条链从没开始；`wlan_func_on_by_chrdev` 等 10 秒超时后返回 EIO。
所有 SELinux `avc: denied` 都是 `permissive=1`，只是审计记录，不是拒绝。

**这正好解释了为什么 dijun 把 WLAN 和蓝牙做在一起**：它配置里的注释写着
「UART 与 BT 回调在用户开启 WLAN 前注册，共享预校准才能完成」。
我们上一轮把**启动链**搬过来了（模块 / loader / nvram.ready / uart_launcher /
wlan_assistant / co_clock_flag），但**没有把蓝牙栈带过来**——而 BT 回调正是
预校准的前置条件。对这颗芯片，蓝牙运行时不是附加功能，是 WLAN 能开机的依赖。

对照 dijun 的 `[modules].load`，我们还缺：
`bluetooth`、`hci_uart`、`hidp`、`rfcomm`、`btmtk_uart_unify`，
以及 `bluetoothd` / `dbus-daemon` / `skyward-bt-bridge` 这些守护进程，
还有 `[[owners]]` 给 `/dev/ttyXR0`、`/dev/stpbt`、`/proc/stpbt/*` 设 bluetooth 账户。
`uart_launcher` 每 24 秒退出，很可能就是因为 BT 侧没有栈可注册。

顺带记录：`conninfra@(conf_parse:512) failed to parse 'coex_wmt_epa_elna'`
说明 `conninfra.cfg` 确实被读到了，这条缺字段是否影响待验证。

### 待确认的修复方案

- **方案 A（最小验证，推荐先做）**：只把 BT 内核模块 + `bluetoothd` /
  `dbus-daemon` / `skyward-bt-bridge` 作为**普通服务**开机拉起（用已提取的
  `.references/bluetooth-runtime/`），让 BT 回调注册，看 `pre_cal_flow` 是否推进、
  `wlan0` 是否出现。**不需要蓝牙 UI**，目标是先让 WLAN 可用。
  同时查 `uart_launcher -f 3` 反复退出的原因（可能需要 bluetooth 用户、
  `/proc/stpbt/bt_uart_launcher_notify` 或 `vendor.bluetooth.*` 属性）。
- **方案 B（完整）**：按 dijun 把蓝牙整套（Rust crate + UI 页 + worker +
  `pack --bluetooth`）合入 0.3.1，WLAN 自然随之可用。工作量大，但一步到位。
- 两者都需重新编译打包；A 更快、变量更少。

## 2. 平板 UI：观察正确，0.3.1 官方没有平板布局

- 全 `crates/` 搜索 `tablet` / `is_tablet` / `two_column` / `split_view` /
  `wide_layout` **零命中**。
- `Device` 结构只有 `codename / brand / model / ab / soc / os_version_prop`，
  **没有 `tablet` 字段**。
- `App::new(start, w, h, px_per_dp)` 是单列页面栈（一套 `stack`、一列行、
  全屏抽屉），没有按宽度分栏的分支。

所以现在就是**把手机的单列布局画在 2136×3200 的画布上**，看起来像手机是必然的。
`[display] width/height/density` 只影响尺寸换算和主题，不改变布局结构。

上游有未合并的 PR 提供平板分栏布局：`#24`（早期）、`#25`（Pad 6S Pro `sheng`）、
**`#44`**（最新，Pad 7 Pro `muyu`，基于 `main ff1668f`），含
`tablet.rs`（左右分栏）、`[device] tablet = true`、`rotation.rs`（旋转动画）等。
但 #44 有 32 文件约 2500 行，且基于 `app.rs` 单文件结构；**0.3.1 已把 `app.rs`
拆成 `app/` 20 个模块**，不能直接套用，需要手工移植分栏渲染逻辑。

### 需要用户确认方向

1. **左右分栏**（PR #44 那种：左侧导航/列表、右侧详情），工作量最大；
2. **仅适配宽屏**：保持单列，但把内容宽度、边距、字号、卡片密度按平板重排，
   改动集中在布局常量，风险最小；
3. **先不动 UI**，优先把 WLAN 修好。

另外如果「不是平板 UI」还包含**方向**问题（当前画布竖屏 2136×3200、
面板横屏 3200×2136、触摸靠 `swapped + reversed=["y"]` 映射），那是另一条线，
需要一并说明想要横屏还是竖屏布局。

## 3. 下一步

按惯例先等确认再改代码。建议顺序：**先方案 A 修 WLAN（顺带验证蓝牙栈能否
拉起）→ 再单独一轮做平板 UI**。若选方案 B，则蓝牙与 WLAN 一起完成。
