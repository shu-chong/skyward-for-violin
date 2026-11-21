# Skyward-violin 全代码审查：错误与冲突清单

日期：2026-11-19 08:45
范围：`C:\Users\shc\Desktop\skyward-violin`，相对官方 v0.3.1 基线 `d1ea3f7`
规模：48 个源文件改动，+2393/-223 行；另有 `tablet.rs` 311 行、`image/protection.rs` 新增文件
方法：3 个子代理分域静态审查（刷机/槽位/AVB、UI 状态机/锁屏/触摸、配置/打包/WLAN/MTP），
由主代理逐条复核关键项；Android release 构建 RC=0、无新增 warning。

**本轮只审查、未改任何代码、未刷机。**

---

## 总览

| 级别 | 数量 | 核心 |
|---|---|---|
| 🔴 严重 | 3 | 自动合并竞态可致 recovery 服务退出；MTP 切换失败丢 adb 且不自愈；自动合并与设计文档/回退路径冲突 |
| 🟡 中等 | 9 | 侧栏漏拦 Wiping；平板侧栏吞双击亮屏；写回失败被吞报成功；自动 MTP 触发判据矛盾等 |
| ⚪ 轻微 | 12 | 死代码、孤儿注释、无测试覆盖、打包器静默脆弱等 |

好消息：**撤销总体干净**——自动切槽（`activate_installed_slot`）、dd 式写回（`write_from_device`）、
密码体系（sha2/Lock）、记住解锁密码，都无功能性残留；`protection.rs`、`hold_required`、
真进度解析、`disable_both` 逻辑自洽；编译无 error/warning。

坏消息：几处"新加的功能没和既有状态机对齐"，其中一处有真实崩溃路径。

---

## 🔴 严重

### S1. 自动合并与手动合并竞态 → recovery 服务退出（可复现路径）

**位置**：`crates/skyward/src/ui.rs:1599-1611`（begin 自动合并）+
`ui.rs:2751-2755`（Effect::Merge 的 `ensure!`）+
`crates/skyward-ui/src/app/act.rs:788-794`（滑动合并按钮）+ `ui.rs:2623`

**问题**：`begin()` 在 `Standing::Waiting` 时自动起 merger，但随后 `ui.rs:1611` 仍
`set_update(standing)`（Waiting），所以"滑动以合并"按钮在合并完成前仍可见。
用户在这 0~2 秒窗口点它 → `Effect::Merge` → `ensure!(self.merger.is_none())` 失败 →
错误经 `turn() → run() → main.rs` 冒泡，**整个 recovery 进程退出**，已解密数据/网络丢失。

**复核**：三处代码已逐字确认，`ensure!` 确为致命错误传播路径。

**建议**：
1. `begin()` 启动 merger 后把状态设为 `Standing::Merging(0.0)` 而非 `Waiting`，按钮随即消失；
2. `Effect::Merge` 改成幂等：`if self.merger.is_some() { report("正在合并"); return Ok(vec![]) }`，
   任何竞态都不再冒泡成致命错误。（两条都做，双保险）

### S2. MTP 切换失败把端口留在 `none`、本会话不可自愈

**位置**：`ui.rs:3201-3226`（enable_mtp_after_decrypt）、`ui.rs:3238-3247`（file_transfer）、
`crates/skyward-sys/src/usb.rs:55-56`

**问题**：`usb::switch("mtp,adb")` 先 `set("none")` 再 `set("mtp,adb")`。若前者成功、后者失败，
`switch` 返回 Err，但代码只写 kmsg，仍保留 `transfer=Some(t)`、`state.mtp=true`，
`resting()` 一直是 `mtp,adb`。后续调用因 `transfer.is_some()` 提前返回，`begin()` 的兜底切换早跑过，
**端口停在 none：adb 没了、MTP 也没起来，且本会话无恢复路径**。这直接抵消了 C 方案
"开机别让 Windows 丢 adb" 的价值。

**复核**：`usb::switch` 的 none 中转、`transfer.is_some()` 早退、失败只写 kmsg 均属实。

**建议**：`switch` 失败立即回滚：`let _ = t.stop(true)`（= `set_mtp(false)` + `switch("adb")`），
再记 kmsg；或有限重试、最终回落 adb。

### S3. 自动合并的设计取舍与仓库文档冲突、且不可逆

**位置**：`ui.rs:1599-1610`；对照 `docs/ALL-FIXES-MASTER-2026-11-17.md` 9.4/9.5

**问题**：文档 9.5 明确写"滑动合并是为开机界面卡住后的补救，为开机去滑会抹掉旧系统退路"，
9.4 又把卡 Mi 归因于 boot HAL 未写 bootable（后经 11-18 调研推翻，改为 OS4.0.6 通病）。
当前 `begin()` 无条件自动合并，每次进 recovery 只要 Waiting 就推进，**等于默认执行了文档警告的
不可逆动作**。虽然 11-18 真机验证它确实解决了卡 Mi，但这是推翻既有取舍的行为改变，
且没有页面/确认告知"合并不可逆"。

**建议**：这是个产品决策，需你拍板：
- 保留自动合并 → 至少在 kmsg 和页面显式告知、限定条件（本次无在途操作、本槽确为 target）；
- 或回到"用户滑动才合并" → 删掉 `else if Waiting` 分支，同时消除 S1 竞态。

---

## 🟡 中等

### M1. `navigation_open()` 漏 `Install::Wiping`，侧栏可在清数据中切页
`app/mod.rs:1368-1391`。拦截列表没有 `Install::Wiping`，但 `back()` 对同一状态明确拒绝。
破坏性操作期间侧栏可绕过规则、隐藏进度。建议加入 `Wiping`；最好 `back()` 与它共用同一判定函数。

### M2. 平板导航列区域（约 17.5% 屏宽）双击亮屏失效
`skyward/src/ui.rs:2588-2593` + `tablet.rs:224-239` + `app/input.rs:12-28`。息屏时触摸仍先
`tablet.route(t)`，落点在导航列返回 `Input::Sidebar`，不会调 `app.touch()`，而双击检测在
`app.touch()` 内。建议息屏且双击开启时绕过 route、直接用 `route_whole()` 把坐标交给 `app.touch()`。

### M3. 写回 recovery 失败被吞，安装仍报成功
`install/run.rs:602,627-648,885`。`reflash()` 返回 `()`，失败只发 `Event::Reflashed(Failed)`，
`update_system`/`run_script` 不检查仍返回成功。而 `hold_required` 把写回变成强制前提、拒绝不写回的安装，
承诺与结果不一致。建议让 reflash 的失败反映到 `Outcome`（加 `reflash_failed` 标记）。

### M4. BLKROSET 方案无重试、真机未复验
`image.rs:208-225`、`image/protection.rs`、`run.rs:640-642`。文档记录的真机 EPERM 推测含
"内核仍持有分区"，当时的修复有 4×750ms 重试；现改为清一次只读标志、无重试。
BLKROGET/SET 只能清块层标志，若 EPERM 来自内核持有或标志被重置则不足，且叠加 M3 会静默失败。
建议真机复现确认；必要时补有限重试。

### M5. 自动 MTP 触发判据矛盾
`ui.rs:2484` 要求 `closed.is_empty()`，而 `enable_mtp_after_decrypt` 和 `offer_storage` 只用
`storage_open()`。owner 已解锁但非 owner 用户仍锁时不启动 MTP、反之却 offer 存储。
建议统一用 `storage_open()`。

### M6. `mtp_after_decrypt` 在不走解锁流程时永不触发
`ui.rs:2450-2451`（`decrypt_told` 开头 `else { return }`）。若用户没进解密页（数据已开/无 FBE），
自动 MTP 永不执行；又因 `state.mtp=false` 隐藏了手动开关，本会话无入口。
建议改为基于 `data.storage_open()` 状态变化触发，`decrypt_told` 作兜底。（需确认 violin 是否必走解锁）

### M7. `conninfra.cfg` 无条件 `rd.add`，stock 含同名文件则打包失败
`pack/main.rs:184`。同文件的 `TASK_PROFILES` 有 `if rd.find().is_none()` 保护，此处没有；
OS4.0.6 stock 变化即构建失败。建议加存在性判断或 replace 合并。

### M8. `logd.rc` 修复是静默 no-op
`pack/main.rs:172-180`。仅当文本恰好含一次 `"    priority 10\n"` 才替换，stock 缩进变化就静默跳过
（注释称否则 logd 每 5s abort 刷屏）。建议未命中时打印警告。

### M9. WLAN "EBUSY 重试"实际对任意错误重试 10 秒
`wlan.rs:657-667`。注释说只对 EBUSY，代码对所有 `Err` 重试，EACCES/ENODEV 被吞 10s 才报。
建议仅对 `raw_os_error()==Some(16)`（或 EAGAIN）重试。

---

## ⚪ 轻微

- **L1** `wake_lock` 是永恒死状态：`power.rs:44-49` 的条件恒真（外层已保证 `!lockscreen()`），字段只写不读。删。
- **L2** 死字段 `State.wlan_password`（`state.rs:44,235`）无任何读写；WLAN 密码实际在 `SavedWlan`。
- **L3** 未使用配置字段 `DeviceConfig.bluetooth`、`Touch.xiaomi_double_tap`（config/lib.rs）；"配置即生效"不成立。
- **L4** 孤儿文档注释：`decrypt.rs:105-106`、`settings.rs:63-64` 是删函数后残留的 `///` 段。
- **L5** `skyward/src/settings.rs:71-73` 注释仍提"screen lock 明文"，该功能已删。
- **L6** `devices/violin.toml:175-179` 注释仍称 `hand_over` 硬失败，但 `main.rs:620-623` 已改容错。
- **L7** settings 文件先 `File::create`（0644）再 `set_permissions(0600)`（`settings.rs:74-79`），
  有短暂明文可读窗口且失败被 `let _ =` 吞。建议 Unix 用 `OpenOptions::mode(0o600)`。
- **L8** `Keeper::Unreadable` 只能靠 `remade()` 恢复（`settings.rs:88-97`），会话内 /metadata 恢复可读后仍不落盘。
- **L9** `Mount::served()` 与字段成死代码（`fuse.rs:315,399-401`），进度改用引擎字节后无生产调用方；`run.rs:737` 注释仍引用。
- **L10** 新状态机零单测：`Screen::Lock`/`wake`/双击检测/`navigation_open`/倒计时均无回归测试。
- **L11** 锁屏页继承 `back: true`（`pages.rs:751`），画出一个点了无效的返回箭头与边缘滑动。
  建议 `Frame { back: false, header: false, .. }`。
- **L12** `DisbaleAvb` 的 locked 检查只认 `ro.boot.vbmeta.device_state`，属性缺失会算成 unlocked；
  代码其它处用 `androidboot.flash.locked`/`androidboot.vbmeta.device_state`。建议统一或缺失时保守拒绝。

---

## 需真机/用户确认的边界

1. **S3 的产品取舍**：自动合并保留还是改回手动滑动？（11-18 真机证明自动合并能解决卡 Mi）
2. **M4**：BLKROSET 清一次标志是否真能消除真机 EPERM？需刷官方包复现一次。
3. **M6**：violin 是否必走解锁流程？若否，自动 MTP 在部分场景不触发。
4. **触摸轴映射**：`swapped` 启发式 + `reversed=["y"]` + rotate270 组合，11-18 后无真机 log，建议复核四角。
5. **状态栏整屏**：`compose_overlays` 画在 whole 上，rotate270 后方向/可读性无截图验证。
6. **`progress_card_named` 放大字号**（11-18 最后改动）：最新镜像是 11-17，**这段代码很可能未经编译/真机验证**；
   标题基线与名字首行基线间距 `22dp - ink_top`，字号变大存在两行重叠疑虑。

---

## 处理结果（09:00 更新）

- **S3 已拍板：保留自动合并**。用户明确表示"已经刷机，自然是要抹掉旧系统"——
  即接受合并的不可逆性。因此 11-18 加的 `Waiting → 自动合并` 保留，不改回手动滑动。
- **S1 已修**（因 S3 保留，S1 必须修，否则等于留着一个崩溃开关）：
  - `ui.rs:1599-1619`：自动起 merger 后把状态设为 `Standing::Merging(0.0)`（不再是 `Waiting`），
    "滑动以合并"按钮随即消失；用 if/else 保证不被末尾的 `set_update(standing)` 覆盖。
  - `ui.rs` 的 `Effect::Merge`：把 `ensure!(merger.is_none())` 改为幂等保护——
    已有 merger 时只写 kmsg 并 `return Ok(Vec::new())`，不再冒泡致命错误。
    （`effect()` 每次只处理一个 Effect，函数内已有 return 先例，写法安全。）
  - Android release 构建 RC=0、无新增 warning（`.references/logs/build-s1fix-20261119-0900.log`）。
  - 尚未打包镜像、未刷机——等其余要修的项一起处理。

## 建议修复顺序

1. ~~**S1**（崩溃，改动最小：两处）— 优先。~~ ✅ 已修（见上）
2. **S2**（丢 adb，C 方案核心失效）— 加失败回滚。
3. **M1**（侧栏漏拦，一行）— 顺手补。
4. **M7/M8/M9**（打包器与 WLAN 的静默错误）— 低风险。
5. **S3 + M3/M4** — 需你拍板设计取舍 + 真机复验后一起做。
6. **L 类清理 + 补单测** — 择机。

本轮未改代码。等你确认要修哪几项，我再动手。
