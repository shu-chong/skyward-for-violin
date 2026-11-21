# 去掉记住密码/自动解密、真倒计时、锁屏文案；MTP 待决策

日期：2026-11-15 23:25
镜像：`out/skyward-violin-OS4.0.5-20261115-2325.img`
SHA-256：`a022ee2be07301627db373668726e0abeb871ed9a17b4bdfe8a4190ed15ad822`
ramdisk：49,181,261 字节 / 104,857,600 字节
承接：`…2312.img`（`9be4b77e…a75f`）

本轮**没有刷机、没有写入任何设备分区**；MTP 本轮**未改动**，见第 4 节，等你选方案。

## 1. 现场 log（`.references/logs/dmesg-20261115-2319.txt` / `logcat-20261115-2319.txt`）

| 项 | 值 |
|---|---|
| `sys.usb.config` / `sys.usb.state` | `adb` / `adb` |
| `persist.sys.usb.config` | `adb` |
| `init.svc.adbd` | `running`（开机 9.68 s 启动一次，pid 343） |
| `ro.adb.secure` | `0` |
| `xr_usb_composite_disconnect` | 0 |
| `underflow` / `signal 6` | 0 / 0 |

**ADB 是通的**（本轮 log 就是经 adb 抓的）。你看到的「电脑识别不到设备」是**没有 MTP 盘符/文件传输设备**，不是整台设备掉线。

## 2. 已按你的要求改掉

### 2.1 去掉「保存解密密码」和「自动解密」功能（⑥ 整体移除）

- 删 `Switch::RememberData`、解密页的开关与整段明文警示、`State.remember_data / data_password`、
  `Settings.remember_data / data_password`、`App.pending_password / auto_tried / auto_unlocking`、
  `App::auto_credential()` / `unlock_succeeded()`、`unlock()` 里的暂存、`unlock_failed()` 里的自动失效、
  `decrypt_told()` 里开机自动 `Job::Unlock` 的那段。
- 现在开机**不会**自动拿密码解 data；每次要解密就手输，密码不落盘。
- **旧文件里的明文也会被抹掉**：`skyward/src/settings.rs::read_file()` 读到老 `settings.json`
  含 `"data_password"` 时返回 `Kept::Nothing`，`Keeper` 约 2 秒后把不含该字段的新设置整份写回，
  并写一条 kmsg `the kept screen lock password is being erased from the settings`。
  旧文件里的 `lock`（salt/hash）也一并被新文件替换掉。
- **⑦ WLAN 自动连接保留**：它存的是 Wi-Fi 密码，不是解密密码。镜像自检里仍有一处
  「密码以明文保存在 /metadata」就是 WLAN 页的说明文字。如果你也要把它去掉，说一声。

### 2.2 锁屏提示删掉后面 8 个字

`LOCK_NOTE` 从「向右滑动解锁 · 仅防误触，不设密码」改为**「向右滑动解锁」**（英文 `Swipe right to unlock`）。
滑块上「向右滑动解锁」、整屏 `whole` 合成、右划即回原页、解密后才启用 —— 都不变。

### 2.3 「3 秒」改成真正的逐秒倒计时

上一版是静态文案「完成 · 3 秒后自动返回」，数字不会动。现在：

- `State.home_left: Option<u32>` 保存剩余整秒；`App::tick()` 每跨一秒更新并 `refresh()`。
- `App::deadline()` 在倒计时期间额外返回 `min(home_at, now+1000)`，主循环每秒重绘一帧。
- 按钮文案用 `tr!` 模板实时渲染「完成 · 3 秒后自动返回」→ 2 → 1，到点回首页。
  点「完成」立即离开并清掉 `home_at / home_left`。

### 2.4 「未挂载」文案修正

`fmt::data()` 把 `Data::Closed / Opening / Failed` 的「未挂载」改成**「未打开」**，
首页机身存储那行也从「data 分区未挂载」改成「data 分区未打开」。

**为什么开机先显示这个、进解密页才显示「已解密」**：Skyward 开机不主动探测 data 分区
（官方 v0.3.1 的设计，`Decryptor::Open` 只在进入解密页/需要 data 的操作时启动），
而这一版又按你的要求去掉了自动解密，所以初始状态就是 `Data::Closed`。进去后解密流程确认状态，
显示「已解密」是正常的。如果你要**开机就自动探测一次状态（不输密码、只更新显示）**，
下一轮我在 `begin()` 里加一次 `Job::Open` 即可 —— 那不属于「自动解密」，但会多一个开机 worker。

## 3. 构建与自检

| 项 | 结果 |
|---|---|
| 宿主 `cargo build --release -p skyward-pack` | RC=0（2.57 s） |
| Android `cargo build --release --target aarch64-linux-android -p skyward` | RC=0（1 m 04 s，无新增 warning） |
| 打包 | RC=0，ramdisk 49,181,261 B |
| 镜像内二进制 SHA | 与本地同为 `d6be96da…89df` |
| 新文案 | 「秒后自动返回」模板、「向右滑动解锁」×2、「未打开」×2、明文清除 kmsg |
| **反向检查为 0** | 「记住解锁密码」「仅防误触，不设密码」「3 秒后自动返回」「未挂载」「自动解密」「the kept screen lock is being tried once」 |
| `skyward-mtp.rc` / `skyward/system-init` | 缺席（MTP 仍关、无双系统） |
| 镜像内配置 | `mtp=false`、无振动、90Hz、rotate 270、columns 560 |
| ramdisk 往返 | `repack` RC=0，49,181,261 B 一致 |

## 4. MTP：诊断结论与三个方案（本轮未改，等你定）

20:36 那轮为了修「开机 15.6 s MTP gadget `none → mtp,adb` 重绑导致 Windows 复合设备重枚举丢 ADB
interface」，把 `devices/violin.toml` 设成 **`mtp = false`**，打包器不再放 `skyward-mtp.rc`、
`prop.default` 追加 `persist.sys.usb.config=adb`、运行时不启动 `Transfer`。代价就是没有 MTP 盘符。

| 方案 | 做法 | 代价 |
|---|---|---|
| **A 恢复 MTP** | `mtp = true`，回到旧行为 | 开机 15.6 s 重绑，Windows 会丢 ADB，需 `adb kill-server && adb devices` 恢复；这是当初关掉它的原因 |
| **B 保持现状**（零风险） | 用 `adb pull/push /sdcard/…` 传文件 | 电脑资源管理器里没有盘符 |
| **C 按需 MTP**（推荐折中） | 开机仍只跑 adb；解密后在设置/高级里加「启用文件传输」按钮，手动点才切到 `mtp,adb` | 重绑只发生在你主动操作时、可预期；开机 adb 不掉。需要下一轮实现 |

选 A / B / C 告诉我即可。若选 C，我会让打包器带上 `mtp.rc`、但 `begin()` 不自动 `Transfer::start()`，
改由按钮触发，并把 USB 切换保持幂等。

## 5. 刷入后验证

1. 解密页**不再有**「记住解锁密码」开关；开机不会自动解密；`adb shell cat /metadata/skyward/settings.json`
   里应不再有 `data_password`（开机约 2 秒后重写）。
2. 解密成功后按钮显示「完成 · 3 秒后自动返回」并**逐秒变 2、1**，到点回首页；点「完成」立即走。
3. 锁屏页 Skyward 下面只写「向右滑动解锁」，没有后半句；右划到底回原页。
4. 状态显示由「未打开」→ 进解密页后「已解密」；双击亮屏、WLAN 自动连接、本地更新、90Hz 无回归。

回退：`out/skyward-violin-OS4.0.5-20261115-2312.img`（`9be4b77e…a75f`）。
只刷 `recovery_b`，`fastboot reboot recovery` 可用。
