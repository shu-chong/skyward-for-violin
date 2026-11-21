# 锁屏改为「右划防误触页」+ 双击亮屏真机验证

日期：2026-11-15 23:12
镜像：`out/skyward-violin-OS4.0.5-20261115-2312.img`
SHA-256：`9be4b77e1821401e23d4305567c12984555bd744a75cf4fadb5aa0915a6da75f`
ramdisk：49,182,119 字节 / 104,857,600 字节；Android 二进制 8,847,600 字节
承接：`…2215.img`（`237b3949…5521`）

本轮**没有刷机、没有写入任何设备分区**。

## 用户反馈与纠正

> 双击亮屏打开开关后可以双击亮屏。
> 锁屏样式理解有问题，类似 twrp ofox，仅一个防误触的功能。右划进度条就能解开。
> 是解密后才启用锁屏样式，不需要密码，解密后不需要再次输入密码。

上一轮把「锁屏」做成了**带密码的 recovery 锁**：4–8 位数字、salt+SHA-256、设置页。
理解错了。正确的样式是 TWRP / OFox 那种**纯防误触的滑动解锁页**：

- **没有密码概念**：不设、不输、不存，不加密，也不需要设置页。
- **右划进度条即解锁**：整屏页面底部一条 `widgets::confirm` 滑轨，拖到底就回到息屏前那一页。
- **只有 data 解密完成后才启用**：`wake()` 的条件是 `Data::Open | Data::Erased`；
  未解密 / 正在解密 / 解密失败时不出现，避免挡住解密页输密码。
- **解密后不再输任何密码**：锁屏页什么也不问。

**⑥ 记住解锁密码保留**：它属于「开机自动打开 data 分区」那一层（`/metadata/skyward/settings.json`
里的 `data_password`），和这层防误触页不是一回事，仍默认关 + 明文警示 + 关闭即清除。

## 真机验证：双击亮屏可用（用户实测 + log 证据）

抓取 `.references/logs/dmesg-20261115-2307.txt`（6,743 行）、`logcat-20261115-2307.txt`（20,179 行）：

| 指标 | 结果 |
|---|---|
| `underflow` | 0 |
| `signal 6` | 0 |
| `xr_usb_composite_disconnect` | 0 |
| `[NVT-36536] … bTouchIsAwake:1` | **43 次** |
| `[NVT-36536] … bTouchIsAwake:0` | **0 次** |
| NVT `mode:1091` / `mode:1071` / `mode:1000` | 23 / 22 / 18 次 |
| WLAN `Link Quality` 持续上报 | 38 行（WLAN 已连） |
| `skyward: violin 0.3.1 recovery up` | 正常 |

`bTouchIsAwake` **全程为 1、没有一次 0**，说明「只关背光、跳过 `surface.rest()/turn_off()`」
这条策略在本机确实让触摸控制器保持供电 —— 这就是双击能亮屏的一手证据，
上一轮报告里「NVT 息屏可能 suspend」的疑虑可以结案。0 underflow 也再次印证 90Hz 稳定。

## 本轮改动（删的多、加的少）

**删掉整套密码体系**：

- `skyward-ui/Cargo.toml` 的 `sha2` 依赖、`settings.rs` 的 `Lock` 结构 / `Lock::set/matches` /
  `Settings.lock` 字段及其 `of()` / `apply()`；`State.lock` 字段。
- `Screen::LockSet`、`Action::Unlock / LockSave / LockOff`、`App::act_unlock() / act_lock_save()`、
  `pages::lock_set()`、设置页「恢复模式锁屏」入口、「关闭锁屏」按钮。
- `App::typing()` / `type_key()` 里锁屏页的键盘分支；`widgets::keypad` 多余的 `enter` 参数回退。
- `App.snapshot` 字段（上一轮为模糊快照预留、实际没用）。
- `wake()` 里的 `state.code.clear()` —— 它会误清解密页正在输入的密码，一并去掉。

**锁屏页改为纯右划**（`pages.rs::lock()`，仍画在整屏 `whole`）：

- 整屏背景 + 锁图标 + `Skyward` + 「向右滑动解锁 · 仅防误触，不设密码」+ 电量。
- 底部一条 `widgets::confirm`，`Confirm::UnlockScreen` 文案改为「向右滑动解锁」，
  进度取 `state.confirm`，拖到底 `App::unlocked()` 弹回息屏前那页。
- 无键盘、无码点、无密码比对、无任何持久化。
- 整屏机制不变：`set_full_screen()` 切画布尺寸、`ui.rs::show()` 直接 compose 到
  `tablet.whole` 并跳过 `assemble()`、`tablet.route_whole()` 让每个触摸落在整屏坐标系。

**启用条件**（`power.rs::wake()`）：

```rust
if dark && matches!(self.state.data, Data::Open | Data::Erased) {
    self.state.confirm = 0.0;
    // push Screen::Lock, set wake_lock, refresh …
}
```

`dark` 排除解密页与锁屏自身；`back()` 仍不许从锁屏返回。设置页「手势与安全」现在只留
「双击亮屏」开关，下面一段说明文字写明「解密完成后每次亮屏会先显示整屏右划解锁页，只防误触、不需要密码」。

**保持不变**：④ 双击亮屏（默认关，已实测可用）、⑥ 记住解锁密码、⑦ WLAN 自动连接、
整屏 `whole` 合成、90Hz / rotate 270 / columns 560、只跑 adb、无双系统。

## 构建与自检

| 项 | 结果 |
|---|---|
| 宿主 `cargo build --release -p skyward-pack` | RC=0（2.64 s） |
| Android `cargo build --release --target aarch64-linux-android -p skyward` | RC=0（1 m 06 s，无新增 warning） |
| `skyward-pack recovery` | RC=0，ramdisk 49,182,119 B |
| 镜像内二进制 SHA | 与本地同为 `4a20ee13…5a` |
| 新文案命中 | 「向右滑动解锁」×3、「仅防误触，不设密码」、「不需要密码」、「双击亮屏」、「记住解锁密码」、「开机自动连接」 |
| **旧密码文案（反向检查）** | 「4 到 8 位数字」「恢复模式锁屏」「密码不正确」「锁屏已设置」「关闭锁屏」「请输入 4 到 8」「salted hash」**全部为 0** |
| `skyward-mtp.rc` / `skyward/system-init` | 缺席 |
| `prop.default` | `persist.sys.usb.config=adb`、`ro.adb.secure=0`（追加行） |
| 镜像内 `device.toml` | `mtp=false`、`supports_vibration=false`、`refresh=90`、`rotate=270`、`columns=560` |
| ramdisk 往返 | `repack` RC=0，49,182,119 B 一致 |

## 刷入后请验证

1. 未解密时息屏 → 亮屏：**不出现**锁屏页，可继续在解密页输密码。
2. 解密完成后息屏 → 电源键或双击亮屏：出现**铺满整屏**的右划解锁页（左侧不露导航列、
   状态栏在整屏顶端、没有键盘、没有码点）。
3. 把进度条拖到最右 → 回到息屏前那一页；中途松手进度归零、不解锁；按返回键不会绕过。
4. 双击亮屏、记住解锁密码、WLAN 自动连接、本地更新、充电动画、90Hz 无回归。

**如果理解还有偏差**：现在的实现是「解密后**每次亮屏**显示」。若你要的是 TWRP 那样
「解密完成那一刻 / 一进 recovery 就先划一次」，告诉我，我在 `decrypt_told()` 里再压一页即可
（会与解密完成 3 秒回首页的倒计时叠在一起，需要顺手调整）。

回退：`out/skyward-violin-OS4.0.5-20261115-2215.img`（`237b3949…5521`，带密码版）
或 `…2058.img`（`c224d747…2244`，无锁屏版）。只刷 `recovery_b`，`fastboot reboot recovery` 可用。
