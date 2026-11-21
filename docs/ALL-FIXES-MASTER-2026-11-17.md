# violin / Skyward 移植：全部 Bug 的根因与修复总览

更新日期：2026-11-17
工作区：`C:\Users\shc\Desktop\skyward-violin`（旧 `skyward` 目录只读保留）
基线：官方 Skyward v0.3.1 + ROM OS4.0.5.0.XOTCNXM（后续刷过 OS4.0.6.0）
设备：Xiaomi Pad 7S Pro（violin，玄戒 XRING O1），Android 15/HyperOS，无 EDL、无振动马达

> 状态标记：✅ 真机验证通过 · 🟡 已改待真机验证 · 🔴 未修复/待定
> 每项的详细过程见文末「专项报告索引」。本报告只做归纳，不重复全文。

---

## 目录

1. [显示与刷新率](#1-显示与刷新率)
2. [触摸](#2-触摸)
3. [解密与开机服务](#3-解密与开机服务)
4. [WLAN](#4-wlan)
5. [振动](#5-振动)
6. [USB / adb / MTP](#6-usb--adb--mtp)
7. [充电与进程崩溃](#7-充电与进程崩溃)
8. [UI 细节与交互](#8-ui-细节与交互)
9. [刷机 / 槽位 / 虚拟 AB / AVB](#9-刷机--槽位--虚拟-ab--avb)
10. [Windows 构建链](#10-windows-构建链)
11. [专项报告索引](#11-专项报告索引)
12. [仍未解决 / 待验证](#12-仍未解决--待验证)

---

## 1. 显示与刷新率

### 1.1 开机黑屏 ✅
- **症状**：recovery 已启动、挂载正常，但点亮屏幕前 UI 退出，黑屏。
- **根因**：`skyward/src/ui.rs::normal_max()` 读取高通专属 sysfs `/sys/module/msm_drm/parameters/dsi_display0` 计算背光上限；玄戒 O1 非高通，路径不存在 → 背光初始化失败 → UI 退出。
- **修复**：`normal_max()` 读不到 msm_drm 参数时回退到 `max_brightness`（不扣 HBM 余量）。
- **证据**：修复后日志出现 `the pages speak Hans`、MTP 起来、无 `recovery failed`。
- **文件**：`crates/skyward/src/ui.rs`
- **旁证**：keymint/gatekeeper(mitee) 的报错与黑屏无关，是解密服务问题（见 3.1）。

### 1.2 144Hz 花屏 ✅
- **症状**：追最高刷新率 144Hz 时花屏、背光保护性关闭。
- **根因**：玄戒 O1 DPU 在 144Hz 下 `dpu_underflow`（日志 903 次）、`last vote: ddr 0 MB/s, clk profile 1`，未开 DSC / 双 pipe / DDR 带宽投票 → 数据下溢。
- **修复**：新增可选配置 `Display.refresh`，`surface.rs::open(path, refresh)` 用 `find(|m| m.vrefresh()==refresh)` 选 mode，并把 `preferred` 重绑到该 mode（上电、`to_fastest()`、熄屏唤醒统一）。`violin.toml` 设 `refresh=90`。
- **证据**：90Hz 真机日志 `new refresh rate is 90(hz)`、`underflow` 0 次；144Hz 那次 903 次 underflow。面板 modes 实测有 `3200x2136x{120,144,90,60,50,48,30}`；HyperOS 只有 60/90/144 三档可用。
- **文件**：`skyward-config`、`skyward/src/ui.rs`、`skyward-ui/src/surface.rs`、`devices/violin.toml`

### 1.3 画面方向颠倒 ✅
- **症状**：竖屏内容显示颠倒/横屏。
- **根因**：`rotate=90` 在真机物理安装方向下是倒的。运行时画布来自 DRM mode `3200×2136`（横屏），toml 的 `width=2136 height=3200` 只是元数据。
- **修复**：`violin.toml` 改 `rotate=270`；`Tablet::place/unplace` 用同一 Rotation，触摸逆变换自动跟随。
- **文件**：`devices/violin.toml`、`skyward-ui/src/tablet.rs`

### 1.4 没有平板 UI（左右分栏）✅
- **症状**：官方 Skyward 无平板布局，全部按竖屏单栏排。
- **根因**：0.3.1 上游没有 tablet 布局；`columns` 字段不存在。
- **修复**：新增 `skyward-ui/src/tablet.rs`（约 311 行）——竖屏逻辑画布 2136×3200 + 常驻左侧导航列 + 90°/270° 旋转 blit + 触摸逆变换与分栏路由。`Display` 加 `rotate`/`columns` 字段。零旋转零分栏时直通，其它机型零开销。
- **配置**：`rotate=270`、`columns=560`。
- **文件**：`skyward-ui/src/tablet.rs{lib,app/mod}.rs`、`skyward/src/ui.rs`、`devices/violin.toml`

---

## 2. 触摸

> 触摸是迭代最多的一类，走过弯路。按最终定位的正确因果排列；中间弯路见 2.5。

### 2.1 触摸完全没有事件 ✅
- **症状**：`event3 NVTCapacitiveTouchScreen` 中断持续增长（4 万次），但 ABS_MT 坐标 20 秒 0 事件；按键正常。触摸 IC 硬件/固件/SPI/中断全正常。
- **根因（关键转折）**：NT36532 驱动**不自己上报坐标**，它把原始帧经 `/dev/xiaomi-touch` 交给用户态 `vendor.xiaomi.hw.touchfeature-service`，由 `/odm/lib64/libtouchreport.so` 转成 input 事件。recovery 没启动该服务 → 读到的帧无人转换。之前所有 ioctl 配置都是错误方向。
- **修复**：`violin.toml` 加 `[[service]]` 启动 `/odm/bin/hw/vendor.xiaomi.hw.touchfeature-service`（root、libraries 含 /system+/vendor+/odm/lib64、`ready_service=vendor.xiaomi.hw.touchfeature.ITouchFeature/default`、`optional=true`）。移除错误的手动 ioctl（`set_mode`/`TouchMode`/`set_touch_mode`）。
- **参考**：kirklin/skyward PR #1（piano，同款 NT36532 + 2136×3200）。
- **证据**：用户确认"触摸可以了"。

### 2.2 触摸 EPERM（手动 ioctl 阶段，已废弃）✅
- **根因**：open `/dev/xiaomi-touch` 后 handle 的 `touch_id=-1`，未先 `SELECT_TOUCH_ID` 就发 `COMMON_DATA_CMD` → 驱动返回 -EPERM。**修复**：ioctl 序列改为 open → `ioctl(0x40005403,0)` 选 id → `ioctl(0xc4085400,{touch_id=0,cmd=0,mode=1000,data_len=1,data_buf[0]=1})`。此路径后来整体废弃（见 2.1），仅作记录。

### 2.3 SPI 0xBF（手动 ioctl 阶段，已废弃）✅
- **根因**：op_mode 命令在驱动重刷固件（3 次 `nvt_update_firmware`）后 1ms 就发，IC 尚未退出 bootloader → 回 0xBF。`xiaomi_touch_set_mode_value` 是 void，不把 SPI 失败传回 ioctl，所以 ioctl 重试无效。**尝试**：加 sleep 2s 有效，但整条路径已废弃。

### 2.4 触摸坐标映射 ✅
- **症状**：触摸能上报但点按位置对不上（横屏画布 vs 竖屏触摸坐标）。
- **根因**：触摸坐标 X0–2136 / Y0–3200（竖屏），显示 3200×2136（横屏），需 90° 旋转 + 轴镜像。
- **弯路**：先按"顺时针 90°"推导设 `reversed=["x"]`，结果变成对角线反转（右下↔左上）。**教训**：不能靠"顺/逆时针"术语猜，要靠实测点角坐标反推。
- **修复**：移植 PR #1 的 `swapped` 自动检测（比较 span(x)/w 与 span(y)/h）+ `reversed` 镜像。实测点右下角得 tx≈100 / ty≈3060 → 正确映射为 `屏幕X=触摸Y`、`屏幕Y=2136-触摸X` → `reversed=["y"]`（与 piano 一致）。
- **文件**：`skyward-config`(Touch.reversed)、`skyward/src/input.rs`(Panel.swapped/reversed + 坐标映射)、`skyward/src/ui.rs`、`devices/violin.toml`

### 2.5 长按滑动断触 ✅
- **症状**：长按滑动时进度条被打断、连按解密密码断触。
- **根因**：PR #30 每秒定时向触摸写 `enable_touch_raw=1`，重置 baseline。
- **修复**：不引入该定时线程（0.3.1 官方与 dijun 都没有）。`enable_touch_raw=1` 只在初始化写一次。
- **证据**：日志 `for the touchscreen again` 0 次。

### 2.6 双击亮屏 ✅
- **症状/需求**：暗屏后双击点亮。默认关。
- **根因/实现**：纯软件双击检测——暗屏记 `Down`，400ms / 80px 内第二次即 `wake()`；不碰 ioctl。开启时 `show()` 只关背光、跳过 `rest()/turn_off()`（保留面板与触摸供电）；关闭时保留原 dijun 解绑路径。
- **证据**：真机实测可用；log 中 `[NVT-36536] bTouchIsAwake:1` 43 次、`bTouchIsAwake:0` 0 次 → 触摸息屏时全程保持唤醒，疑虑结案。
- **文件**：`skyward-ui/src/app/{power,input}.rs`、`skyward/src/ui.rs`

### 2.7 息屏后触摸不能亮屏（早期）✅
- **症状**：电源键锁屏后触摸不亮，延迟 5s 才亮。
- **根因**：`ACTIVE=false` 在玄戒上 EINVAL。
- **修复**：先降级到 Resting，后用 dijun 完整 plane/connector 解绑恢复（`surface.rs::turn_off()` 在 `lp.is_none()` 时清空 plane `FB_ID/CRTC_ID`、connector `CRTC_ID`、清 `MODE_ID`，唤醒时成对恢复）。

---

## 3. 解密与开机服务

### 3.1 data 分区解密失败 ✅
- **症状**：keymint/gatekeeper(mitee) exit 1，`/data` 未挂载。
- **根因链**：① 手动跑报 `libvndksupport.so not found`（libhardware.so 依赖）；该库在 `/mnt/system/system/lib64`，`libdl_android.so` 在 `/mnt/system/system/lib64/bootstrap`，不在服务默认 LD_LIBRARY_PATH。② Skyward 的 `libraries` 即 LD_LIBRARY_PATH。
- **修复**：keymint/gatekeeper 的 `libraries` 补 `/mnt/system/system/lib64` + `/mnt/system/system/lib64/bootstrap`。全部服务设 `optional=true`（未 ready 只记日志、不终止 recovery）；sharedsecret 协商失败也只记日志。
- **证据**：补路径后 mitee 解密链跑通，`/data` 解密成功（`/dev/block/dm-10 holds f2fs at /data`）。

### 3.2 开机慢（>1 分钟）✅
- **根因**：`services.start()` 串行，每个 optional 服务未 ready 等满 `READY_TIMEOUT=20s`；keymint 约 31s、gatekeeper 约 51s 超时，累计 40s+。
- **修复**：根本解决靠 3.1 让 mitee 服务 ready；optional 服务未就绪不终止。（未做并行启动优化）

### 3.3 fstab 不匹配玄戒 ✅
- **根因**：Skyward 找标准 `fstab.*`，violin 是 `fstab.O1_asic`（在 /vendor/etc 或 /odm/etc）。
- **修复**：`skyward-block/src/fstab.rs` 动态扫描所有 `fstab.*`；移植 dijun `preserve_o1_data()`——去掉 `check/formattable/resize`，persist/nvdata 改 `ro,noload`，移除 `rescue→/cache`，保证不格式化 /data 与 /metadata。

### 3.4 「未挂载」文案错误 ✅
- **症状**：解密页显示"未挂载"，实际是 Skyward 开机不主动探测 data（初始 `Data::Closed`）。
- **修复**：文案改为"未打开"。

### 3.5 记住解锁密码 / 自动解密 —— 按用户要求移除 ✅
- **背景**：曾实现"默认关+明文警示"的记住密码与开机自动解密。
- **用户决定**：全部删除。
- **修复**：移除 `Switch::RememberData`、`remember_data`/`data_password` 字段、`auto_credential()`、开机自动 `Job::Unlock`；旧 `settings.json` 里的明文密码在下次启动时被检测并整份重写清除（约 2s 后，写 kmsg）。settings 文件加 0700/0600。

---

## 4. WLAN

WLAN 的修复跨越多个层次，最终靠"按官方时序一次性加载全部驱动"打通。以下按因果层次列出（含走过的弯路结论）。

### 4.1 cfg80211 缺 rfkill 依赖 ✅
- **根因**：`cfg80211.ko` 报 `Unresolved symbol rfkill_*`（err -2）。rfkill.ko 在 system_dlkm，但 vendor_dlkm 的 modules.dep 没声明 cfg80211→rfkill 依赖。
- **修复**：`[wlan].modules` 或 `[modules].load` 显式把 `rfkill` 放最前。

### 4.2 固件下载触发 = WMT write "1" ✅
- **根因**：MT6639 经 `/dev/wmtWifi` 接受命令——写 `"1"` 触发 `WIFI_write_on`（下载固件 + 注册 wlan0），写 `"0"` 关，写 `"WR-BUF:NVRAM"` 写 NVRAM。recovery 缺 `conninfra_loader` 与写 "1"，驱动 probe 成功但 wlan0 不注册。
- **修复**：`[modules].load` 加 `conninfra`；加 `[[service]] conninfra_loader`（oneshot，root）；`[wlan].writes` 加 `{path="/dev/wmtWifi", value="1\n"}`。

### 4.3 NVRAM 缺失导致 Invalid pointer ✅
- **根因**：写 "1" 报 `I/O error (os error 5)`、驱动报 `Invalid pointer`——缺 NVRAM（MAC + 校准数据）。
- **修复**：`[partitions].mounted` 加 `nvdata`；加 `[[service]] wlan_assistant`（daemon、optional、root），读 `/mnt/vendor/nvdata/APCFG/APRDEB/WIFI`（6144B），经 `/dev/wmtWifi` 的 `WR-BUF:NVRAM9` 写入（`Set NVRAM state[1]`）。
- **证据**：Invalid pointer 消失。

### 4.4 PCIe EP 未上电（次生现象，非独立 bug）✅
- **现象**：`xring-pcie ce400000.pcie: EP not powered up yet`。**结论**：这是芯片未上电时的预期行为，`xring_pcie_linkup_retry` 会在 WIFI_on 后重建链路；是 4.3 未解决时的次生现象，不是独立根因。停止了这条错误方向的深挖。

### 4.5 pre-cal 超时 = BT 内核模块缺失 ✅
- **根因**：MT6639 Wi-Fi/BT 共享芯片，`g_pre_cal_mode=4` 的预校准需 BT 与 WIFI **两侧**都向 conninfra 注册 `pre_cal` 回调。我们只加载了 WLAN 侧模块，`bt=[NULL][NULL]` → `pre_cal_flow:0` → `wlan_func_on_by_chrdev: Timeout`。`btmtk_uart_unify.ko` 在模块初始化时注册 `btmtk_pre_cal_cb`（内核态动作）。
- **修复**：`[modules].load` 补 5 个 BT 内核模块：`bluetooth / hci_uart / hidp / rfcomm / btmtk_uart_unify`。**不需要** BlueZ/D-Bus/BlueALSA 用户态栈或蓝牙 UI。
- **证据**：`bt=[有值][有值]`、`pre on DONE!!`、`BT turn on ok!`、`wlan0` 创建。

### 4.6 时序错误（WLAN 曾反复打不开的真正执行问题）✅
- **根因**：走偏的是**执行时序**。官方 ROM 把 `post-fs-data: insmod conninfra` → `start conninfra_loader` → `on property:vendor.connsys.driver.ready=yes: insmod wmt_chrdev + wlan_drv, start wlan_assistant` 在开机几秒内连续完成；我们把驱动放到用户点 Wi-Fi（65s）才加载，中间空约 50s，且 wlan_assistant 在 `/dev/wmtWifi` 建立前就启动。官方 `modules.load` 开机就加载全部 wlan 模块（非按需 insmod）。
- **修复**：整套 wlan 模块移到 `[modules].load`（rfkill 第一）；services 顺序改 conninfra_loader → wlan_assistant；`[wlan].modules=[]`；写 "1" 保留。
- **注意**：recovery 的 init 只解析 `/system/etc/init/*.rc`，不解析 vendor 第二阶段 rc，必须自己按官方顺序复现。

### 4.7 点开 WLAN 打不开（bring_up 上电短路）✅
- **根因**：`wlan.rs::bring_up()` 把上电写入 `cfg.writes` 放在 `if !net.exists()` 内。开机 pre-cal 已创建 `wlan0` 但随后芯片断电（接口残留、`operstate=down`），用户点开时 `net.exists()==true` → 整个上电分支被跳过 → 没写 "1" → 芯片没电。
- **修复**：把 `cfg.writes` 移出 `if !net.exists()`，无条件上电；模块/固件仍只在接口不存在时加载。MTK WMT 的写 "1" 幂等，`off_writes` 写 "0" 与之配对。

### 4.8 WLAN 关闭时未交还电源 ✅
- **修复**：`wlan.rs` 关闭时执行 `off_writes`（`wmtWifi=0`）。

### 4.9 WLAN 开机自动连接 🟡
- **实现**：默认关；连接时把 SSID(hex)+密码存进 `settings.json`（复用 0.3.1 持久化）；开启后 `begin()` 发 `On`+`Join`（复用 EBUSY 重试）；`wpa_supplicant.conf` 加 `country=CN`/`disable_scan_offload=1`。
- **证据**：自动连接生效，`192.168.31.173`、`reaches the internet`。

### 4.10 检查更新报 "no ro.product.mod_device" ✅
- **根因**：OS4.0.6.0 包不再提供 `ro.product.mod_device`，而 `ui.rs::installed()` 把它当必填。
- **修复**：mod_device 优先、缺失回退 `ro.product.device`；契合 `region()` 的"无后缀=CN"逻辑。

---

## 5. 振动

### 5.1 设置里有振动项但机器无马达 ✅
- **根因**：violin 无振动马达（DT 里 `cs40l26@43 status=disabled`、`xring,xr-haptic` 空节点、所有 input 设备无 FF_RUMBLE）。上游设置页无条件显示振动项。
- **修复**：`config.Device` 加 `supports_vibration`（默认 true），`violin.toml` 设 false。`State.has_vibrator` + `vibrates()` 统一所有 `Effect::Haptic` 判定；设置页整段跳过振动项。
- **结论**：不做完整音频驱动栈（振动走 cs40l26，在 `xring_audio_hw_third.ko`，依赖栈大）。

---

## 6. USB / adb / MTP

### 6.1 adb「只能充电」/ 进来一会就断 ✅
- **根因（设备侧正常）**：开机 15.6s 时 `sys.usb.config=none → mtp,adb` 的 gadget 重绑（`xr_usb_composite_disconnect`）让 Windows 复合设备（MTP MI_00 + ADB MI_01）重枚举，WPD 一侧没把 ADB interface 带回来。设备侧 `sys.usb.state=mtp,adb`、`adbd=running` 始终正常，log 本身就是 adb 抓的。
- **修复（方案 B）**：打包器可选跳过 `MTP_RC`，USB 只跑单一 `adb` function → 开机无重绑。新增 `DeviceConfig.mtp`，`violin.toml` 设 `mtp=false`。
- **零改动规避**：禁用 WPD、关 USB 选择性暂停、不打开盘符、`adb kill-server && adb devices`、命令带序列号。
- **结论**：刷机掉 adb 不会黑砖——刷 recovery 走 fastboot 通道，只写 recovery 分区，bootloader 完好。无 EDL，坚持只刷 `recovery_b`。

### 6.2 恢复 MTP 盘符（方案 C）✅
- **用户要求**：开机只跑 adb，**data 解密完成后自动**切到 `mtp,adb`（不要手动按钮）。
- **难点**：`skyward-mtp.rc` 必须打包（否则 `/dev/usb-ffs/mtp` 不存在、MTP 起不来），但开机不能启动 Transfer。
- **修复**：新增 `DeviceConfig.mtp_after_decrypt`（默认关），`violin.toml` 设 true；打包器在 `mtp||mtp_after_decrypt` 时放 rc，但属性追加条件仍是 `!cfg.mtp`；`ui.rs` 新增 `enable_mtp_after_decrypt()`，在 `decrypt_told()` 且 `closed.is_empty() && data.storage_open()` 时启动 `Transfer` → `usb::switch(resting())` = `mtp,adb` → `offer_storage()` 把 /sdcard 给电脑。
- **证据**：`sys.usb.state=mtp,adb`、`file transfer came up once data was decrypted`、`mtp: the computer configured the function/opened a session`。
- **代价**：切 MTP 必经 `none` 重绑，adbd 被 SIGKILL 再重启，Windows 可能在解密完成这一刻丢 adb（`adb kill-server && adb devices` 恢复）。

---

## 7. 充电与进程崩溃

### 7.1 插拔充电器后黑屏 / 掉 adb / data 未解密 / 触摸 WLAN 全失效 ✅
- **症状**：插拔充电器瞬间 `Service 'recovery' received signal 6`（SIGABRT），新进程缺密钥/输入/WLAN 状态，连锁表现为黑屏、掉 adb、要重新解密。
- **根因 A（SIGABRT）**：`compose_charging()` 复用了页面尺寸的 `dither`（按 detail 约 1576 宽创建），但充电动画已提到整屏 `whole`（2136 宽），`glow::paint_band()` 按整屏宽度索引 → 越界 panic。另有 `info.battery.expect()` 与 `charge::Art::new().expect()` 在插拔瞬间可能 panic。
- **修复 A**：`App` 新增独立 `overlay_dither: Option<(u32,u32,glow::Dither)>`，按覆盖画布尺寸创建；`info.battery=None` 时跳过充电层；`Art::new` 失败记日志不 panic；`charge_canvas` 加尺寸检查。
- **根因 B（EACCES 连锁重启）**：旧进程 abort 后 DRM master 未释放（`dpu_lastclose` 需时间），新进程 `KmsSurface::open()` 首次 `ALLOW_MODESET` commit 被拒 `EACCES(13)` → 再次退出 → 5s 后才重启。
- **修复 B**：`KmsSurface::open()` 加最多 8 次、每次 250ms 的有限重试，只对 `PermissionDenied/WouldBlock/InvalidInput` 重试，写 kmsg。
- **证据**：真机反复插拔正常、充电动画正常，`received signal 6` 0、`recovery failed` 0。

---

## 8. UI 细节与交互

| # | 症状 | 根因 | 修复 | 状态 |
|---|---|---|---|---|
| 8.1 | 充电动画只在右栏、息屏右侧先黑 | 覆盖层画在 `tablet.detail`，左侧导航列独立绘制 | `compose()` 拆 `compose_pages`（页面）+`compose_overlays`（覆盖层）；`Tablet` 加 `whole` 逻辑整屏，`show()` 顺序改 compose→assemble→overlays(whole)→turn→present | ✅ |
| 8.2 | 状态栏只在右半边 | 状态栏画在 `compose_page()`(detail) | 移到 `compose_overlays()` 画在 whole；`status_bar()` 右端由 `m.w` 改 `c.w` | ✅ |
| 8.3 | 顶部时间/ Skyward 标题重复 | 分栏下主页 Frame.title 非空 | 分栏时 title 置空、header=false；morph 动画用 `frame.title` | ✅ |
| 8.4 | About 缺署名 | 上游无 | `AUTHOR` 改 `© 2026 Kirk Lin · @爱看小说的书虫` | ✅ |
| 8.5 | 无马达却有振动设置 | 上游无条件显示 | 见 5.1 | ✅ |
| 8.6 | 锁屏理解偏差 | 上一轮做成"带密码的 recovery 锁"，用户要的是 TWRP/OFox 式纯右滑防误触 | 删整套密码体系（sha2/Lock/键盘/码点/设置项）；锁屏页只保留整屏右滑进度条；仅 Data::Open/Erased 时启用；不要求密码 | ✅ |
| 8.7 | 锁屏提示多余 8 字 | 文案含"仅防误触，不设密码" | 只保留"向右滑动解锁" | ✅ |
| 8.8 | "3 秒"不是倒计时 | `home_at` 只定时不重绘 | 新增 `home_left`，`deadline()` 每秒唤醒重绘，按钮文案 3→2→1 | ✅ |
| 8.9 | 长包名被截断（1 秒安装名缺前后） | `Canvas::text` 超宽截断加省略号；包名拼在标题里 | 新增 `widgets::progress_card_named()`：标题"正在安装"独占行，名字逐字符按 `text_width` 硬折行，进度行+条 | 🟡 |
| 8.10 | 安装完成页侧栏点不了 | `navigation_open()` 用 `screen()!=Installing` 一刀切，把 `Install::Finished` 也拦了 | 按 `state.install` 细分：仅安装进行中拦，Finished/失败/擦除/结束放行 | 🟡 |
| 8.11 | 进度条"1 秒 100%" | `apply()` 用 `mount.served()`（FUSE 从包读的字节）当进度，FUSE 瞬间读完 8.77GB | `ota.rs` 新增 `Log::Progress`，从引擎日志 `bytes downloaded A/B` 解析真实写入字节；`run.rs` 改用它；kmsg `progress now follows the bytes update_engine wrote` | ✅ |

---

## 9. 刷机 / 槽位 / 虚拟 AB / AVB

### 9.1 刷机时侧栏能点走页面 ✅
- **根因**：键盘/返回键已拦，但平板侧栏走 `tablet::Input::Select` 直接调 `select_section()`，无忙碌检查。
- **修复**：`App::navigation_open()` + ui.rs 输入分发，安装中忽略侧栏选中。（后由 8.10 细化为"仅安装进行中拦"）

### 9.2 官方 OTA 报 "Operation not permitted" 写回 recovery ✅（部分）
- **现象**：系统刷成功（`Update successfully applied`），仅 Skyward 写回 `recovery_b` 报 EPERM。
- **根因**：OTA 结束瞬间平台对 recovery 分区的保护 / update_engine 刚退出内核仍持有分区。yarp 能成功是因为它用裸 `dd` 从"仍保留 recovery 的槽"复制到"被覆盖的槽"，且写前先卸载分区。
- **修复（B 方案）**：`keep.rs` 加 `source()`（找仍持有 Skyward 的槽）与 `write_from_device()`（块设备→块设备复制，等价 dd）；`reflash()` 改用它，`Ok(false)` 回退内存写；加 4 次×750ms 重试应对瞬时 EPERM。
- **状态**：🟡 已构建待真机验证（`…0022.img`）。

### 9.3 切槽（走过的弯路，已撤销）✅
- **弯路**：曾误判"Skyward 没切槽"导致卡 Mi，加 `activate_installed_slot()` 在成功后 `set_active()`。
- **真相**：日志显示 update_engine 自己 15:42:31 就 `SetActiveBootSlot(B)`；官方明确"更新中 slot 由 update_engine 管，别碰"。我们的调用是多余的。
- **修复**：撤销该函数与调用。

### 9.4 卡 Mi 反复重启（最新定位：boot HAL 变体）🔴
- **现象**：刷完官方包重启卡 Mi；但手动"滑动合并"后能开机；yarp 刷完直接重启就能进系统。
- **确凿差异（两份 log 对比）**：
  - yarp 启动 `android.hardware.boot-service.xring.**recovery**`（recovery 专用 HAL），切槽时执行 `ab_slot_bootable_write(0,1)`，把新槽标 bootable → 正常启动。
  - Skyward（`violin.toml:166`）启动普通版 `/vendor/bin/hw/android.hardware.boot-service.xring`，注册了服务但**没有** `ab_slot_bootable_write` 记录 → 新槽 active 但 bootable 不对 → bootloader 拒绝启动、卡 Mi。
- **拟修复**：把 `violin.toml` 的 boot-service path 换成 `.recovery` 变体。**未做**——设备持续掉线，无法确认该二进制的确切文件名；盲目改风险大。
- **此前的相关发现**：虚拟 A/B 更新处于 `Unverified`（state=`08 02`），页面显示"更新等待合并"；完成页原本无合并提示，已补（见 9.6）。

### 9.5 "更新等待合并"是什么 ✅
- **机制**：Android 虚拟 A/B——新系统以快照写进新槽（`/metadata/ota/snapshots/` 有 system_b/product_b/...），新槽首次启动成功后自动"确认+合并"，更新才闭环。合并前状态 `Unverified`。
- **结论**："滑动以合并"是给"系统能启动但合并失败"的补救，不是开机前置条件；为开机去滑它反而抹掉旧系统退路。卡 Mi 是"系统起不来导致自动合并前提不成立"，不是合并功能坏。

### 9.6 安装完成页补合并提示 🟡
- **根因**：官方/dijun/onyx 完成页只说"重启后从该槽位启动"，不提合并——上游文案缺口，用户把"卡首次启动"误判为刷机失败。
- **修复**：`finished()` 的 `Outcome::Installed` 分支，`state.update` 为 `Installed/Waiting/Compressed` 时在"重启到系统"下加三语提示（首次启动自动合并、勿强制重启、卡开机界面回 recovery 手动合并）。

### 9.7 去掉 EDL ✅
- **根因**：violin 无 EDL；上游有高通专用 EDL 重启项。
- **修复**：`reboot.rs/pages.rs/search.rs` 移除 EDL（`TARGETS` 6→5 等）。

### 9.8 禁用 AVB 验证按钮 🟡
- **原理**：vbmeta 头偏移 120 的 4 字节 flags：`bit0=HASHTREE_DISABLED`(dm-verity)、`bit1=VERIFICATION_DISABLED`(bootloader 校验)。yarp commit `15bb3f70` 只是让禁用时同时清两位写 vbmeta_a/b。Skyward `avbctl` 底层早有相同能力，只缺 UI。
- **实现**：`avbctl.rs` 加 `pub disable_both()`（locked 拒绝、一次读写清两位）；`Screen::Avb`+`Confirm/Effect::DisableAvb`；高级页加"禁用 AVB 验证"（红盾）→ 子页说明+警示+滑动确认；search 登记。**按用户要求无恢复按钮**。
- **验证点**：滑动后 flags 应变 `00 00 00 03`。

### 9.9 mod_device / 包名折行 / 进度 —— 见 4.10 / 8.9 / 8.11

---

## 10. Windows 构建链

| 问题 | 根因 | 修复 |
|---|---|---|
| 0.3.1 官方仅支持 Unix | 多个模块直接用 unix API | `ramdisk.rs/mount.rs/erase.rs/skyward-sys/lib.rs` 加 `#[cfg(unix)]` 隔离 + 非 Unix 桩 |
| 配置校验误判 | Windows 下 `Path::is_absolute()` 把 `/system/...` 判为相对 | 5 处改 `starts_with('/')` |
| 缺 lz4 CLI | 打包需 legacy LZ4 ramdisk | 下载官方 win64 `lz4.exe` 到 `tools/`，打包时加 PATH |
| cargo check 卡死 | MSVC 编译 ring 偶发挂起 / rustc 僵住 | 停掉，改用 release + Android target + 显式 NDK r29 工具链 |
| logd 每 5 秒 SIGABRT | stock `logd.rc` priority 10 | 打包期改 `priority -2` + `rlimit nice 22 22` |
| 新配置字段被静默忽略 | 0.3.1 config 不 `deny_unknown_fields` | 升级时把 `bluetooth/refresh/reversed/optional/off_writes` 全部补回 |

---

## 11. 专项报告索引

| 主题 | 文档 |
|---|---|
| 早期移植（黑屏/花屏/触摸/解密/WLAN 底层） | `docs/PLAN-violin-0.2.0-OS4.0.5.md`、旧工作区 `.workbuddy/memory/2026-11-03.md`、`2026-11-07.md` |
| dijun 合并 + v0.3.1 基线 | `BUILD-v0.3.1-dijun-port-2026-11-14.md`、`UPGRADE-skyward-v0.3.1-2026-11-14.md`、`REVIEW-dijun-skyward-2026-11-14.md` |
| 旋转/分栏/WLAN BT 模块 | `BUILD-rotate-columns-wlan-2026-11-14.md`、`DIAG-wlan-tablet-2026-11-14.md` |
| 四症状分诊 | `FIX-triage-2026-11-15.md` |
| 充电崩溃 + UI 整理 | `FIX-ui-and-charge-crash-2026-11-15.md` |
| onyx 功能 ①②③ / ④⑤⑥⑦ | `BUILD-sequence-1-3-2026-11-15.md`、`BUILD-sequence-4-7-2026-11-15.md`、`REVIEW-onyx-upstream-2026-11-15.md` |
| 锁屏改右滑 + 双击亮屏 | `FIX-lock-swipe-only-2026-11-15.md` |
| 去记住密码/倒计时 | `FIX-no-remember-countdown-mtp-2026-11-15.md` |
| adb-only / MTP 方案 | `BUILD-no-mtp-adb-only-2026-11-15.md`、`BUILD-mtp-after-decrypt-2026-11-16.md` |
| 进度条 + AVB 三方案 | `FIX-progress-real-bytes-2026-11-16.md`、`DIAG-log-avb-commit-2026-11-16.md` |
| 刷机/槽位/mod_device | `FIX-slot-switch-moddev-notice-2026-11-16.md`、`DIAG-ota-install-2026-11-16.md` |
| 撤销切槽 + 合并提示 | `FIX-remove-slot-and-merge-hint-2026-11-17.md` |
| reflash dd 式 | `FIX-reflash-dd-style-2026-11-17.md` |
| 禁用 AVB 按钮 | `FEATURE-disable-avb-button-2026-11-17.md` |

---

## 12. 仍未解决 / 待验证

1. **🔴 卡 Mi 的根本修复**：boot HAL `.recovery` 变体（9.4）——需确认设备二进制名后改 `violin.toml`。这是当前最关键未闭环项。
2. **🟡 reflash dd 式**（9.2）：`…0022.img` 待真机刷官方包验证。
3. **🟡 禁用 AVB 按钮**（9.8）：待验证 flags 变 `00 00 00 03`、新槽能否自动合并开机。
4. **🟡 包名折行 / 完成页侧栏**（8.9/8.10）：已在 `…2350.img` 及之后，待真机看布局。
5. **🔴 蓝牙 UI / 音频栈**：底层 BT 内核模块已加（为 WLAN），但完整蓝牙功能需 `skyward-bluetooth` crate + BlueZ/BlueALSA + UI + worker，Windows 无法交叉编译该栈；运行时 ARM64 二进制已提取到 `.references/bluetooth-runtime/`。
6. **🔴 在线 GitHub 更新**：需自有 Release 渠道。
7. **🔴 verity 自动合并的最终定性**：日志被 recovery 启动冲掉、pstore 为空，未拿到内核 panic 坐实"verity 全开导致首次启动校验失败"这一推断。

---

*本报告由现有 27 份专项报告与 5 份工作日志归纳而成。所有镜像均在 `out/`，校验值见各专项报告。*
