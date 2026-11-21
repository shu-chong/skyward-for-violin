# 官方仓库更新 & Remoparrot/skyward-onyx 功能评估

日期：2026-11-15 20:45
本轮**纯只读调研**：下载补丁/文档、读 diff、写报告。未改任何源码、配置、镜像，未连设备、未刷机。
onyx 补丁与文档副本：`.references/onyx/patches/`

## 1. 官方 kirklin/skyward 有没有更新

| 项 | 结果 |
|---|---|
| 最新 tag | **`v0.3.1` = `bd34c054ba86`**，仍是最新；我们本地就是这个基线 |
| `main` 最新 | `c5e09f6eb6`（2026-11-14 10:52），比 v0.3.1 **多 2 个提交** |
| 有无新版本 | **没有新 tag / release** |

（此前升级记录里的 `beef595` 是 annotated tag 对象的 SHA，`bd34c05` 才是 commit；两者不矛盾。）

### main 多出来的 2 个提交

1. `cc68936` — dada 设备专属（第二颗 Peach 驱动、secure element 节点），对 violin 无用。
2. `c5e09f6` — **PR #56「wait for a file the modules just loaded make before it is handed over」**，对 violin **有直接价值**：
   - 新增 `skyward_sys::wait_file(path, timeout)`；
   - `main.rs::hand_over()` 在 chown 之前等待模块创建的设备节点出现；
   - 我们之前正因为 `/dev/stpbt`、`/proc/stpbt/*` 只在 BT 驱动 probe 后出现、而 `hand_over()` 对每个节点都是 `chown?` 硬失败，**主动放弃了 `[[owners]]`**（见 `docs/BUILD-rotate-columns-wlan-2026-11-14.md`）。合入这个改动后就能安全补上 BT 节点权限。

同一批还有内部重构，整体升 main 会连锁：`services::once()` 去掉 `libraries` 参数、
`keys::start()` 去掉参数、`skyward-programs::{naming,phone,secure}.rs` 与 `devices/dada.toml` 改动。

**建议**：不整体升 `main`。只挑 `wait_file()` + `hand_over()` 那一段手工移植（约 20 行），
代价小、收益明确；其余是设备专属或无害重构。onyx 也仍停在 `v0.3.1 + 6 个补丁`，没有升 main。

## 2. onyx 仓库是什么

`Remoparrot/skyward-onyx` = Redmi Turbo 4 Pro（onyx，第四代骁龙 8s / SM8735，1280×2772 竖屏，NT38771 触摸，AW8697 马达），
**基于同一个 `v0.3.1 / bd34c05`**，仓库只有 716 KB —— 是**补丁 + 脚本 + 文档**，不是完整 fork：

| 补丁 | 大小 | 内容 |
|---|---|---|
| `0001-windows-host-build` | 6 KB | Windows 宿主构建（用 `--no-default-features` 关掉 `described` 子命令） |
| `0002-onyx-ce-fallback` | 6 KB | user 0 CE 的 gatekeeper protector 回退 |
| `0003-onyx-config-wlan-haptics` | 18 KB | 配置 / WLAN / 振动 / 密钥服务库路径 |
| `0004-onyx-updates-downloads` | 32 KB | 512 并行下载、自更新、**本地 recovery.img 刷入** |
| `0005-onyx-ui` | **158 KB** | 锁屏、双击亮屏、下载页、首页读数、AVB 页、设置项等（约 2100 行） |
| `0006-onyx-version` | 5 KB | 版本号 |

硬件差异大：onyx 是高通手机竖屏、有马达、`qca_cld3_wcn7750` WLAN + `wpa-aidl-shim`；
violin 是玄戒 O1 平板、旋转成竖屏 + 左右分栏、无马达、MT6639 WLAN + 标准 wpa_supplicant。
**补丁不能整包 apply**，只能挑通用逻辑手工移植。

## 3. 六项功能逐项评估

### A. 高级 > Skyward 更新：从本地 `recovery.img` 更新 —— **P0，最值得先做**

- **后端几乎现成**：我们 0.3.1 已有 `Screen::Update`、`Action::UpdateSkyward`、
  `crates/skyward/src/updates.rs`（366 行）的 `write()`：
  `upgrade::unzip(path)` → `upgrade::check()` → `upgrade::slots()` → `upgrade::write()`。
- onyx 的 `0004` 只加了一个 `UpdateSkywardFile { path, codename }` 任务与
  **`write_raw()` ≈ 20 行**（把 `upgrade::unzip(path)` 换成 `std::fs::read(path)`，其余复用
  `write_image()`），再加更新页 UI 的一个「从本地 recovery.img 刷入」按钮与文件选择。
- **完全不依赖硬件**（写的是 A/B 的 recovery 分区，由上游 `upgrade`/`skyward-vab` 处理槽位）。
- **对本项目特别实用**：以后出新镜像可以直接把文件放到 `/sdcard/Download/recovery.img`，
  在 recovery 里选它刷入，**不用每次连电脑 fastboot**，也绕开 Windows WPD 掉线。
- 待确认：裸 `recovery.img` 走 `upgrade::check()` 的校验口径（onyx 用裸镜像可行），
  以及只写当前槽 `recovery_b`、不动 a 槽的安全边界。

### B. 双击亮屏 —— **P1，纯软件，但息屏策略是最大变量**

onyx 的实现（`0005` 的 `app/input.rs`）**不依赖任何触摸固件 ioctl 或高通 SSC**：

```rust
if !self.display_on {
    if self.state.double_tap {
        if let Touch::Down(x, y) = ev {
            if let Some((at, lx, ly)) = self.last_tap.take() {
                if now - at <= 400ms && |x-lx| <= 80 && |y-ly| <= 80 {
                    self.wake(now); self.drag = Drag::Waking; return vec![];
                }
            }
            self.last_tap = Some((now, x, y));
        }
    }
    return vec![];
}
```

配套：`State.double_tap`（默认 true）、设置页开关、`Drag::Waking` 防误触。
这比 dijun 的 `/dev/xiaomi-touch` + `Touch_Doubletap_Mode=14`（NT36536 是否支持未验证）通用得多，
而且正是你之前提过的「自动息屏后触摸屏幕不会亮屏」需求。

**但前提成本高**：onyx 在 `ui.rs` 里**息屏只把背光降到 0、面板保持 On，删掉了
`surface.rest()` / `surface.turn_off()`**（他们的面板 rest 后 `present()` 会黑屏），
并靠 `Wakeful` 保持输入设备在息屏时仍打开。我们 violin：

- 当前息屏走的是 dijun 的**完整 plane/connector 解绑 + 唤醒恢复**，刚修好 EINVAL、真机验证息屏/亮屏正常；
- 玄戒是 video-mode 面板 + DDR 带宽投票，改成「只关背光」等于放弃已验证的路径；
- NVT 触摸控制器在息屏时有 `nvt_ts_suspend`，触摸中断能否唤醒 CPU 未验证；
- 我们刚改过充电层 / `rotate=270` / 整屏状态栏，动 power/sleep 路径有回归风险。

**建议**：单独一轮，做成 `[touch] double_tap`（默认关），先只验证「息屏保留输入 + 不 suspend」，
再接双击；不要和别的功能一起改。

### C. recovery 锁屏 —— **P1/P2，通用 UI，但要手工摘 2100 行里的那一小块**

- 纯状态机，无硬件依赖：`Screen::{Lock, LockSet}`、`wake_lock`、滑动解锁、
  4–8 位 recovery 密码（salt + SHA-256，存 `/metadata/skyward/settings.json`）、
  上一页模糊快照（1/10 采样 + 盒式模糊）、设置项「锁屏背景模糊」。
- `wake()` 是唯一入口：点亮时若当前页不是 `Lock` 就压一个 `Lock` 并置 `wake_lock`；
  解锁后弹回息屏前那页。**和双击亮屏共用同一条 `wake()` 路径，建议一起做**。
- 必须摘的：`pages.rs` 的 `lock()` / `lock_sheet()` / `lock_swipe()` / `lock_set()` /
  `lock_confirm()`，`app/mod.rs` 的 `Screen::Lock` 路由、`app/power.rs` 的 `wake()` 压栈、
  `State` 的锁屏字段、`settings.rs` 持久化。
- **必须适配我们的旋转+分栏**：锁屏要画在整屏 `whole`（像充电层那样），
  否则只会覆盖右栏 —— 这是我们刚踩过的坑。
- `0005` 里还混了下载页、二维码、AVB 禁用页、首页读数、颜色校正等，**不要整包合**。

### D. WLAN 自动连接 —— **P2，框架已有，补的是「开机自动」**

- 0.3.1 官方 `wlan.rs`（724 行）已有 saved profiles / 重连框架；onyx `0003` 补的是：
  - `iface::set_up` 的 **EBUSY 重试**（接口先于驱动就绪）—— 零风险，可单独先合，**对我们也可能有用**；
  - `wpa_supplicant.conf` 的 `country` / `disable_scan_offload=1` / `sae_pwe=2`（5 GHz 扫描）；
  - `[wlan] preload` 的 **AIDL shim 是 onyx 高通专属，violin 不需要**；
  - 开机自动开 WLAN 并加入上次网络，密码存 `/metadata/skyward/settings.json`（0600）。
- 自动连接要处理「用户没主动开 WLAN 时是否该自动开」的语义，改动中等。
- 我们 WLAN 现已能连（`link speed=3000/3000`），这项是体验增强而非修复。

### 在线 GitHub 更新 / 512 并行下载

`0004` 的大部分（512 连接 Range 分段、DNS 缓存、`[update].skyward` 指向
`Remoparrot/skyward-onyx` Release）依赖**我们自己的 Release 仓库**，当前没有；
且 450 行网络代码与本轮目标无关。**建议先只做 A 的「本地文件」分支**，在线更新等以后有发布渠道再说。

### E. 记住解锁密码（下次自动解密）—— **需要你明确拍板的安全决策，不建议默认照搬**

onyx 的实现（`0005` 的 `app/decrypt.rs` / `app/state.rs` / `ui.rs`）：

- `State.remember_data` / `State.data_password`、`Switch::RememberData`；
- 解密成功后把密码写进 `Settings`；下次开机 `auto_credential()` 直接返回
  `(user 0, 密码)` 并发 `decrypt::Job::Unlock`，开机自动解密。

**好消息：持久化机制官方 0.3.1 已经自带，不需要动我们的只读加固。**
`crates/skyward/src/settings.rs` 本来就用 `/metadata/skyward/settings.json`：
`write()` 里 `skyward_vab::metadata_writable(true)` 临时可写、写完立刻恢复只读；
`with_metadata()` 自己把 `/metadata` 挂成只读再卸载。我们的 `preserve_o1_data()`
对 `/metadata` 只去掉了 `check|formattable|resize`，**没有**改成 `ro,noload`
（只有 `persist`/`nvdata` 改了），所以复用上游即可，功能上可行、无硬件依赖。

**但风险很实在，必须说清楚**：

- 密码是**明文**。onyx 自己的设计文档写着「recovery 内 adb 本来就是 root，
  这个锁只防『误看』，不防『拿到手机的人』」「锁屏不是安全边界」。
- 我们 violin 的 `ro.adb.secure=0`、adb 是 root 且**无需授权**；刚为了稳定关掉 MTP，
  但 `adb pull /metadata/skyward/settings.json` 照样能读。锁屏密码通常就是手机解锁密码，
  泄露影响远超 recovery 本身。
- 价值链也更长：onyx 这项是为「下次自动解 **user 0 CE**（相册/微信）」，
  而我们 violin 连 user 0 CE 是否解开都还没验证（见第 4 节）。

**建议三选一，需要你决定**：

1. **不做**（最保守）—— 当前每次手输密码，解密已正常工作；
2. 做，但**默认关闭 + 开关旁明确写「密码以明文保存」+ 一键清除**；
3. 不存明文，改存解密后的 CE 密钥材料 —— 要改 `skyward-crypto`，工作量大，另开一轮。

### F. 解密完成倒计时 —— **零风险小改动，建议顺手做**

`app/decrypt.rs` 的 `start_home_countdown(now)` + `deadline()`：data 打开后
「完成 · N 秒后进入主界面」，3 秒自动回首页，点「完成」可立即走。
纯 UI、不碰数据/分区、无硬件依赖，接我们已有的 `deadline()` 机制即可。
建议和 A 放在同一轮。

## 4. 顺带发现（不在你问的功能里，但值得记）

1. **user 0 CE 解密我们没有验证过**：onyx 的 `0002` 是因为高通设备上 `/data` 设备级解密成功、
   但相册/微信目录仍是密文，才加的 gatekeeper protector 回退。我们一直只确认「data 已解密」，
   **从没检查过 `/sdcard/DCIM/Camera` 的文件名是否可读**。violin 是玄戒 mitee TEE，
   不一定需要 onyx 的补丁，但建议做一次只读检查（进 recovery 后 `adb shell ls /sdcard/DCIM/Camera`）。
2. **库路径一致**：onyx `0003` 补的 `/mnt/system/system/lib64/bootstrap` 等，
   与我们之前自己修解密时补的路径完全一样，互相印证。
3. **Windows 构建的更轻做法**：onyx 用 `skyward-pack --no-default-features` 关掉 `described`
   子命令（它依赖 unix-only 的 `skyward-programs`），比我们把 `skyward-sys` 整个模块
   `#[cfg(unix)]` 隔离更轻。当前构建链已稳定，不必改，可作为以后简化参考。
4. **PR #56 的 `wait_file()`** 能解掉我们「不敢加 `[[owners]]`」的历史包袱（见第 1 节）。

## 5. 建议顺序（等你确认再动手）

| 顺序 | 内容 | 理由 |
|---|---|---|
| **①** | **本地 `recovery.img` 更新**（A）+ **解密完成倒计时**（F） | 都通用、改动小、风险低；前者省掉每次 fastboot，后者纯 UI |
| ② | 官方 PR #56 的 `wait_file()` + 补 BT 节点 `[[owners]]` | 约 20 行，顺手清掉历史包袱 |
| ③ | WLAN 的 `iface::set_up` EBUSY 重试 | 独立小改动、零风险 |
| ④ | **双击亮屏**（B）单独一轮 | 要改息屏策略，必须独立验证，默认关 |
| ⑤ | **锁屏**（C）与双击一起做 | 共用 `wake()`，且要适配整屏 `whole` |
| ⑥ | **记住解锁密码**（E） | **先做安全决策**：默认不做；若做，必须默认关 + 明文警示 + 一键清除 |
| ⑦ | WLAN 开机自动连接 / 在线 GitHub 更新 | 依赖发布渠道与语义决策，最后 |

**不建议**整体升官方 `main`，也不建议整包 apply onyx 的 0004/0005。
**需要你先回答**：记住解锁密码选「不做 / 默认关并警示 / 另开一轮改存密钥材料」哪一种？
另外建议顺手做一次只读检查：`adb shell ls /sdcard/DCIM/Camera`，确认 user 0 CE 是否真的解开。

回退与既有镜像不变；当前交付仍是 `out/skyward-violin-OS4.0.5-20261115-2036.img`
（`fafe86d9…da5`）。
