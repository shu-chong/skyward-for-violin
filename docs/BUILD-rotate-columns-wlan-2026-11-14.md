# violin：竖屏画布 + 左右分栏 + WLAN 的 BT 回调补全（2026-11-14）

本轮交付镜像：`out/skyward-violin-OS4.0.5-20261114-1140.img`
SHA-256 `dfa73fd55313c93b3083f2c6487560d4ace0ccae590aa87699102733612145f5`
104,857,600 字节（ramdisk 49,166,113），生成于 11:40:42。

基线：官方 `skyward v0.3.1` + ROM `OS4.0.5.0.XOTCNXM`。本轮**没有连接设备、
没有刷机、没有写入任何设备分区**。

---

## 1. 用户的选择

- WLAN 走方案 **A**：不做蓝牙 UI/音频，只补 BT 侧让 WLAN 能上电。
- 平板 UI：**画布竖屏 2136×3200 + 左右分栏**（旋转 90°，不是保持横屏）。

## 2. 三个改变做法的调研结论

### 2.1 运行时画布其实是 3200×2136 横屏

`/sys/class/drm/card0-DSI-1/modes` 的 7 档全是 `3200x2136`。运行时画布来自
`KmsSurface::size()` → `mode.size()`，即 **3200×2136**。
`[display] width=2136 height=3200` 只是设备规格元数据（`skyward-info`、
`skyward-preview` 用），**不决定运行时画布**。上一轮报告里把它说成"画布竖屏"
不准确，这里更正。

所以「竖屏画布」必须靠软件旋转实现：页面按 2136×3200 逻辑画布排版，
送到物理横屏帧缓冲时转 90°；手指坐标反向转回来。

### 2.2 WLAN 打不开的根因是内核 BT 模块一个都没加载

`logs/20261114-1028/kernel.log`：

```
14.367  connv3_core_subsys_ops_reg [pre_cal] type=[WIFI] ... pre_cal=[有值]
14.367  connv3_core_subsys_ops_reg [pre_cal] type=[1] bt=[NULL][NULL] wf=[有值]
17.847  WIFI_write 1 → get_pre_cal_status: pre cal status: 0
27.860  wlan_func_on_by_chrdev: Timeout, pre_cal_flow: 0 → WMT turn on WIFI fail
```

MT6639 是 Wi-Fi/BT 共享芯片，`g_pre_cal_mode=4` 的预校准必须由 **BT 与 WIFI
两侧都向 conninfra 注册 `pre_cal` 回调**才能推进。我们 `[modules].load` 里
只有 rfkill/conninfra/wmt_chrdev/wlan_drv + 4 个触摸模块，**没有加载任何 BT
内核模块**；同 O1 的 dijun 加载了 `bluetooth / hci_uart / hidp / rfcomm /
btmtk_uart_unify` 五个。

决定性证据：`btmtk_uart_unify.ko` 里有 `btmtk_pre_cal_cb`、
`btmtk_pre_cal_pwr_on_cb`、`btmtk_connv3_sub_drv_init` —— **BT 驱动在模块
初始化时就向 conninfra 注册回调**，是内核态动作。因此：

> A 方案不需要 BlueZ / D-Bus / BlueALSA 用户态栈，不需要蓝牙 UI，也不需要
> `skyward-bt-bridge` 去打开 `/dev/stpbt`。补内核模块即可。

`uart_launcher` 每 ~24 s 退出（`pid 379/430/432/433`，exit 0）正是因为
`/proc/stpbt/bt_uart_launcher_notify` 等节点由 BT 驱动 probe 后才出现，
没有模块时它做完就退。

dijun 那句「UART 与 BT 回调在用户开启 WLAN 前注册」指的就是这条链；
上一轮只搬了启动链、漏了 BT 内核模块，这才是 pre-cal 卡住的原因。

### 2.3 为什么不加 `[[owners]]`

`main.rs::hand_over()` 对每个 path 做 `chown?` + `set_permissions?`，
**节点不存在就硬失败**，会让 recovery 开不了机。而 `/dev/stpbt`、
`/proc/stpbt/bt_uart_launcher_notify` 只在 `btmtk_uart_unify` probe 之后才
出现，且 procfs 常拒绝 chown。本构建所有服务跑 root、SELinux permissive
（日志 `permissive=1`），无需改属主。已验证前几版不带 owners 也能读到
NVRAM（`Set NVRAM state[1]`）。

---

## 3. 代码改动

| 文件 | 改动 |
|---|---|
| `crates/skyward-ui/src/tablet.rs` | **新增**。竖屏逻辑画布 + 常驻导航列 + 90°/270° 旋转 blit + 触摸逆变换与分栏路由 |
| `crates/skyward-ui/src/lib.rs` | 导出 `tablet` 模块 |
| `crates/skyward-ui/src/app/mod.rs` | 新增 `App::select_section()`：侧栏选页直接从栈根切换，不带滑动、不堆叠 |
| `crates/skyward-config/src/lib.rs` | `Display` 加 `rotate`、`columns` 两个字段（serde default） |
| `crates/skyward/src/ui.rs` | `Pages` 持有 `Tablet`；`App::new` / `Canvas::keep_ready` 用逻辑页尺寸；`compose` 画进 `tablet.detail`，`tablet.paint` 合成旋转后 `present`；触摸事件经 `tablet.route` 分发 |
| `devices/violin.toml` | `rotate = 90`、`columns = 560`；`[modules].load` 补 5 个 BT 模块；更新注释 |

### 关键设计

- **已验证的触摸映射层一行未动**：`Inputs::open` 仍传物理 `(3200, 2136)`，
  `swapped + reversed=["y"]` 保持原样。旋转/分栏是在事件分发处加的一层
  逆变换（`Rotation::unplace` → 减去导航列宽），与 `paint` 里的
  `Rotation::place` 严格互逆。
- **全部可配置、改配置重打包即可，不用重编译**：
  - `rotate = 90 / 270` 翻转方向；`0` 关旋转
  - `columns = 0` 关左右分栏
  - `reversed = ["x"] / [] / ["x","y"]` 调整镜像
- 零旋转 + 零分栏时 `Tablet::paint` 走 `copy_from_slice` 直通，其它机型零开销。
- 侧栏 12 个分区入口（主页/搜索/WLAN/安装/备份/恢复/挂载/解密/清除/高级/
  设置/重启），按下高亮、跨分隔线手势不串栏；按下时 `app.redraw()` 触发重绘。
- 每帧多一次 3200×2136 像素的旋转搬运——这是用户选项里已说明的性能开销。

## 4. 构建与自检

| 项 | 结果 |
|---|---|
| `cargo build -p skyward-pack`（宿主，rustc 1.100.0） | RC=0 |
| `cargo build --target aarch64-linux-android -p skyward` | RC=0，8,810,104 字节 |
| `skyward-pack stock --config` | RC=0 |
| 打包 | RC=0，ramdisk 49,166,113 字节 |
| `repack` 往返 | 1049 条 ramdisk 条目逐条一致 |
| 镜像内 `system/bin/recovery` | 8,810,104 字节，与本地产物一致 |
| `skyward/system-init` | 缺席（无双系统） |
| `logd.rc` | `priority -2` + `rlimit nice 22 22` |
| `conninfra.cfg` | `co_clock_flag=1` |
| 镜像内配置 | `refresh=90`、`rotate=90`、`columns=560`、13 个模块（含 5 个 BT） |

## 5. 刷入后请重点验证（这一轮是**方向验证轮**）

1. **画面方向**：若上下颠倒或左右反了，把 `rotate` 改成 `270` 重打包即可。
2. **左右分栏**：左侧导航列 + 右侧页面是否同时可见；点侧栏能否切页。
3. **触摸坐标**：这是最大风险点。原先 `swapped + reversed=["y"]` 是在
   **未旋转**的横屏画布上标定的，旋转 90° 后可能需要改成 `["x"]`、`[]`
   或 `["x","y"]`。请点几个已知位置（比如右下角）确认，把落点告诉我。
4. **WLAN**：看 `connv3_core_subsys_ops_reg [pre_cal] bt=` 是否从 `[NULL][NULL]`
   变成有值、`pre_cal_flow` 是否推进、`wlan0` 是否出现、`wlan_func_on_by_chrdev`
   是否还 Timeout。
5. **90Hz / 息屏 / logd / 长按触摸**：确认上一轮已验证的修复没有被回归。

## 6. 回退

- 已验证的上一版（无旋转、无分栏、无 BT 模块）：
  `out/skyward-violin-OS4.0.5-20261114-0954.img`（`cb08d374…`）。
- 原厂 recovery：`001_DNA/recovery.img`（`376631cf…30293`）。
- 只刷 `recovery_b`，保留 `recovery_a` 原厂版；`fastboot reboot recovery` 在本机
  实测可用。

## 7. 本轮未做

- 蓝牙 UI / 配对 / A2DP 音频栈（BlueZ、BlueALSA、D-Bus）。运行时已提取到
  `.references/bluetooth-runtime/`，但要移植 `skyward-bluetooth` crate、UI 页、
  worker 与打包器 `--bluetooth`，另起一轮。
- 双击亮屏（`NT36536` 是否支持 mode 14 未验证）。
- 144Hz（需 DSC / 双 pipe / DDR 带宽投票 + 持续 present，见 9.18）。
