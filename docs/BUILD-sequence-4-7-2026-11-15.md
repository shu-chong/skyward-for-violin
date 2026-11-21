# 按 onyx 评估的顺序推进：第 ④⑤⑥⑦ 项

日期：2026-11-15 22:15
镜像：`out/skyward-violin-OS4.0.5-20261115-2215.img`
SHA-256：`237b3949c598cf95c940a2a9ccb2c1577377042ff8adcfcf2f5b9aa183c95521`
大小：104,857,600 字节（ramdisk 49,188,846），Android 二进制 8,847,600 字节
承接：`…2058.img`（`c224d747…2244`），即已完成 ①本地更新+倒计时 / ②`wait_file` / ③WLAN EBUSY 的版本

本轮**没有刷机、没有写入任何设备分区**。用户已拍板 ⑥ 选「默认关 + 明文警示 + 一键清除」，
并要求 ④⑤ 一起做（共用 `wake()`，锁屏必须画在整屏 `whole`）。

## 抓取的 log 与基线判断

用户「已经进入最新的 skyward，抓取 log 查看」。设备为 recovery 模式、`sys.usb.state=adb`：

| 文件 | 行数 |
|---|---|
| `.references/logs/dmesg-20261115-2113.txt` | 8,436 |
| `.references/logs/logcat-20261115-2113.txt` | 14,426 |

结论（**上一版 2058 的现场，不是本轮新镜像**）：

- `underflow` 0 次、`signal 6` 0 次、`xr_usb_composite_disconnect` 0 次 → 90Hz、充电崩溃修复、只跑 adb 三项均无回归，开机没有 USB 重绑。
- WLAN 链路上电正常：`[pre_cal] >>>>>>> pre on DONE!!`、`wlan0` MAC 已分配、connsys power on。
- `skyward: the settings are kept`，以及 `/metadata/skyward/settings.json` 的 read/create/rename 审计行（`permissive=1`，是审计不是拒绝）
  → **/metadata 持久化在真机上确实可用**，这正是 ⑥⑦ 的前提，不需要动 `preserve_o1_data()` 的只读加固。
- `skyward: wrote 1 to /sys/class/touch/touch_dev/enable_touch_raw`，触摸节点正常；电池 51% 充电中，无异常。
- 未发现与 ④⑤⑥⑦ 相关的报错（这些功能尚未刷入）。

## ④ 双击亮屏（默认关）

- `State.double_tap` 默认 `false`，设置页「手势与安全」里开关，副标题写明代价。
- `App::touch()` 暗屏分支：记录 `Touch::Down` 的 `(now, x, y)`，第二次落在
  **400 ms / 80 px** 内就 `self.wake(now)` 并置 `Drag::Waking`；其余情况只记第一下。
  纯软件判定，不依赖 ioctl / SSC，也不写 `enable_touch_raw`（避免 PR #30 每秒重写导致 baseline 重置断触）。
- **息屏策略可切换**（`ui.rs::show()`）：
  - 开关**关**（默认）：保持现有 dijun 完整 plane/connector 解绑路径（`surface.rest()` → 5 s 后 `turn_off()`），零回归。
  - 开关**开**：`panel == None` 时只关背光，**跳过 `rest()` 与 `turn_off()`**，面板保持 On、触摸继续供电，
    这样双击才有事件可读。黑屏帧仍照旧写 0，`wake()` 后 `dirty` 触发重绘。
- **未经真机证实的点**：NVT 触摸在本机息屏时是否会自己 suspend。默认关就是为此；
  开启后若收不到事件，关掉开关立刻回到已验证的路径，不会黑屏、不掉 adb。

## ⑤ 锁屏（与 ④ 共用 `wake()`，整屏 `whole`）

- `Screen::Lock / Screen::LockSet`、`Confirm::UnlockScreen`、`Action::Unlock / LockSave / LockOff`。
- `wake()` 是**唯一入口**：暗屏点亮时，若 `state.lock.is_some()` 且 data 已解密，压入
  `Screen::Lock` 并置 `wake_lock`；解锁后弹回息屏前那一页（`App::unlocked()`），`back()` 不许从锁屏返回。
  **没设密码时锁屏不启用**，电源键亮屏行为完全不变 —— 避免每次亮屏都要多滑一次。
- **整屏实现**（本轮核心，避开「只在右栏」的坑）：
  - `App::set_full_screen(w,h)` / `restore_page_size()`：切 `self.w/self.h` 到 `tablet.logical_size()`，
    清 `frame / stale / spots / dither / overlay_dither / charge_canvas / cols` 缓存；离开锁屏恢复 detail 尺寸。
  - `ui.rs::show()`：锁屏时 `compose_pages(&mut tablet.whole, …)` → **跳过 `assemble()`**（否则导航列会盖回来）
    → `compose_overlays(&mut tablet.whole, …)` → `turn()`。状态栏、充电层、息屏幕布依旧整屏。
  - 输入：`tablet.route_whole()` 只做逆旋转，**不减 sidebar 宽、不命中导航列**，锁屏页的 hits 与绘制同处整屏坐标系。
- 密码：`sha2 0.10.9` 的 salt + SHA-256（`Lock::set/matches`），明文密码从不落盘；
  设置页 4–8 位数字，未设密码时锁屏显示「滑动以解锁」；另提供「关闭锁屏」一键移除。
- 为控制风险**没有引入模糊快照**：不新增 `Ctx.backdrop`（省掉 13 处构造点）、不动 `Canvas`，
  锁屏用主题整屏背景 + 锁图标 + 电量；`widgets::keypad` 只多一个 `enter: &str` 参数。

## ⑥ 记住解锁密码（默认关 + 明文警示）

- `Settings` 新增 `double_tap / lock / remember_data / data_password / wlan`；
  `State` 对应字段，`remember_data` 默认 `false`。
- 解密页的开关**副标题直接写明**：
  「⚠ 密码以明文保存在 /metadata；本机 adb 为 root 且无需授权，任何能接上 USB 的人都能读取。关闭开关即清除。」
  **关闭开关 = 一键清除**（同时清 `pending_password`）。
- 流程：`unlock()` 暂存输入 → `decrypt_told()::Unlocked(Ok)` 才 `unlock_succeeded()` 写回密码；
  `decrypt_told()::Opened(Ok)` 且 `closed` 非空时 `auto_credential()` 只触发一次 `Job::Unlock{user:0, credential}`。
  密码错误或已失效时 `unlock_failed()` 自动关掉开关并清空，**不会反复尝试、不会留下无用的明文**。
- `crates/skyward/src/settings.rs::write_file()` 加 `#[cfg(unix)]` 的 0700/0600（尽力而为，失败不致命），
  不改 `/metadata` 的挂载加固。
- 未做 onyx 的「改存 CE 密钥材料」；价值链（user 0 CE 是否解开）仍未验证。

## ⑦ WLAN 自动连接（默认关）

- `State.wlan_auto` 默认 `false`，WLAN 页「开机自动连接」开关，副标题同样写明密码明文保存。
- `Effect::JoinWlan` 里 `App::remember_wlan()` 把 SSID（hex）、名称、安全类型、密码写入
  `/metadata/skyward/settings.json`（复用 0.3.1 自带的 `Settings` 持久化与 `Keeper`）。
- `ui.rs::begin()` 在开关打开且有保存网络时，向 worker **顺序**发 `Command::On` 再 `Command::Join`，
  复用上一轮已合入的 `iface::set_up` EBUSY 重试；不开开关就不在开机拉起 WLAN，不影响启动耗时。
- `wpa_supplicant.conf` 追加 `country=CN`、`disable_scan_offload=1`（原有 `pmf=1 / sae_pwe=2` 不变）。
  **没有**引入 onyx 高通专属的 `[wlan] preload` AIDL shim。
- 在线 GitHub 更新未做：依赖自有 Release 仓库，当前没有；本地 `recovery.img` 更新（①）已可用。

## 构建与自检

| 项 | 结果 |
|---|---|
| 宿主 `cargo build --release -p skyward-pack` | RC=0（2.53 s） |
| Android `cargo build --release --target aarch64-linux-android -p skyward` | RC=0（58.57 s，8,847,600 B） |
| 打包 `skyward-pack recovery` | RC=0，ramdisk 49,188,846 B |
| `skyward-mtp.rc` / `skyward/system-init` | 均缺席（保持只跑 adb、无双系统） |
| `prop.default` | `persist.sys.usb.config=adb`；packer 追加 `ro.adb.secure=0`（stock 的 `=1` 在前，后者生效） |
| 镜像内 `skyward/device.toml` | `mtp=false`、`supports_vibration=false`、`refresh=90`、`rotate=270`、`columns=560` |
| `system/etc/init/logd.rc` / `conninfra.cfg` | `priority -2` + `rlimit nice 22 22` / `co_clock_flag=1` |
| 二进制一致性 | 镜像内 `system/bin/recovery` 与本地产物 SHA-256 同为 `5a052ca2…6b11` |
| 新代码标记（镜像内二进制） | 「双击亮屏」「记住解锁密码」「开机自动连接」「密码以明文保存在 /metadata」(×2)「恢复模式锁屏」「滑动以解锁」「the saved WLAN is being joined again」「the kept screen lock is being tried once」全部命中 |
| ramdisk 往返 | `repack` RC=0，49,188,846 B / 1047 条一致 |
| 文件清单 | `list` RC=0，1047 条 |

构建坑：宿主 `cargo check -p skyward-ui` 又卡死 11 分钟（本机 MSVC 路径老问题，rustc 子进程僵住、
`target/debug` 无新产物），已停掉，改用历史验证过的 **release + Android target + 显式 NDK r29
LINKER/AR/CC/AR** 链路，一次通过。清单 `.references/logs/build-android-20261115-2212.log`、
`build-pack-20261115-2214.log`、`.references/logs/image-list.txt`。

stock 底稿仍是 `D:\DNA_WIN-230828\…\001_DNA\recovery.img`（`376631cf…30293`）。

## 刷入后请按顺序验证

1. 开机 log 仍 `underflow=0` / `signal 6=0` / `xr_usb_composite_disconnect=0`，adbd 只起一次，
   `sys.usb.state=adb` 全程；90Hz、旋转 270、触摸、充电动画、本地更新、3 秒倒计时无回归。
2. 设置 → 手势与安全：**双击亮屏默认关**。开启后电源键息屏 → 双击玻璃能否点亮；
   若无效，关掉开关即回到原路径（不会黑屏、不会掉 adb）。这一步是唯一未经真机证实的点。
3. 设一个 4–8 位锁屏密码 → 息屏 → 亮屏，确认**锁屏铺满整屏**（左侧不再露导航列、状态栏在整屏顶端），
   输错有提示、输对回到息屏前那页；「关闭锁屏」后亮屏不再要密码。
4. 解密页：**记住解锁密码默认关**；开启并成功解密 → 重启 recovery 看是否自动解密；
   关开关即清除；`adb shell cat /metadata/skyward/settings.json` 检查 `data_password`。
5. WLAN 页：**开机自动连接默认关**；连一个网络后开启 → 重启 recovery 看是否自动连上，
   `/data/vendor/wifi/wpa_supplicant.conf`（或运行时目录）含 `country=CN` / `disable_scan_offload=1`。

回退：`out/skyward-violin-OS4.0.5-20261115-2058.img`（`c224d747…2244`）
或 `…2036.img`（`fafe86d9…da5`）。只刷 `recovery_b`，`fastboot reboot recovery` 本机实测可用。

## 仍未做

蓝牙 UI / 音频栈（BlueZ 无法在 Windows 交叉编译，运行时材料在 `.references/bluetooth-runtime/`）、
在线 GitHub 更新（需自有 Release 渠道）、改存 CE 密钥材料、锁屏背景模糊快照。
