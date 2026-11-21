# 安装完成页侧栏放行 + 卡 Mi 根因诊断（B 槽 verity 被官方包重开）

日期：2026-11-16
镜像：`out/skyward-violin-OS4.0.5-20261116-2350.img`
SHA-256：`15ff95f531be360b488738608fb4b2b66a276597047df61632e69982f3aa3586`
本轮**未刷机、未写设备分区、未改 vbmeta**。

---

## 1. 安装完成页侧栏点不了 —— 已修

### 症状

刷完官方包，完成页只剩"重启到系统"能点，左边导航按钮点不动。

### 根因

`App::navigation_open()`（`app/mod.rs`）判断 `screen() != Screen::Installing` 才放行。
安装完成态 `Install::Finished` 仍属于 `Screen::Installing`，于是连完成页也一起拦了。
这是 23:01 那轮"安装中禁止侧栏切页"修复的副作用——拦得过头。

### 改法

`navigation_open()` 改为按 `state.install` 细分：**只有安装真的在进行中**
（Starting/Opening/Locked/Waiting/Checking/Choosing/Confirming/Updating/Flashing/
SystemChoosing/Systeming/Scripting）才拦；完成/失败/擦除/结束态放行。
"重启到系统"按钮本来就走页面 `Hit`（`Action::RebootTo`），此前可用；现在侧栏也恢复。

---

## 2. 点"重启到系统"仍卡 Mi —— 切槽没问题，是 B 槽系统起不来

### 证据链（后台 log `.references/logs/live-ota2-20261116-233625.txt` + 真机只读检查）

1. **切槽成功且真的生效**：
   - log：`15:42:34 skyward: slot B starts next after the install`
   - update_engine 自己也在 `15:42:31 SetActiveBootSlot: set slot 1 as active`
   - 重启后真机 `ro.boot.slot_suffix = _b` —— **确实从 B 槽启动了**。
   - 所以"没切槽"不是这一轮卡 Mi 的原因，上一轮的切槽修复是对的。
2. **`RebootTo(System)` 路径不改槽**：只是 `power()` → `skyward_sys::power(Power::System)`
   → `adb reboot,`（普通重启），不会把活动槽改回 A。
3. **B 槽 vbmeta 状态变了**（决定性差异）：
   - 当前真机只读：`avbctl get-verity` = **enabled on _b**，`get-verification` = **enabled on _b**。
   - 而 09:58 那次"yarp 切槽能正常开机"时的记录是 `verity is disabled on slot _b`。
   - 这次刷的官方完整包把 B 槽 vbmeta 重写成 verity+verification 都开启。
4. 系统崩溃日志无法读取：recovery 启动时 dmesg 被覆盖，`/sys/fs/pstore/` 为空，
   所以拿不到内核 panic 行。当前 dmesg 是 recovery 自己的（`[libfs_avb] Success`、
   `DM_DEV_STATUS failed for system_ext_b` 等都是 recovery 未挂载动态分区的预期现象）。

### 判断

B 槽的系统镜像本身是刷成功的（`Update successfully applied` / `Installed { slot: B }`），
但 **vbmeta 恢复成 verity+verification 全开** 后，玄戒/HyperOS 这套启动链很可能校验不过 →
反复重启回 recovery。这与"之前关着 verity 时切到 B 能开机"的现象一致。
这是**推断**，没有得到内核 panic 的直接证实（日志被冲掉），不排除其它原因（如 B 槽 boot 链本身问题）。

### 可选方案（都需你拍板，本轮未动）

- **A**：先做一次只读验证——你在 Skyward 里用 `adb shell avbctl disable-verity` +
  `disable-verification`（只改当前槽 B 的 vbmeta flags，可逆），再重启，看能否从 B 进系统。
  若成功，证实是 verity 导致。
- **B**：若 A 有效，把"关闭 verity+verification"做成官方包安装完成后的自动一步
  （对应你最早给的 TWRP commit 意图），并保留一键恢复。写 vbmeta 属启动关键分区，需你确认。
- **C**：不改 vbmeta，接受现状——刷完官方包后用 yarp/fastboot 手动处理启动。

---

## 3. 验证

- Android release RC=0（1 m 02 s，无新增 warning）；宿主 pack RC=0；打包 RC=0；
  镜像内二进制 SHA 一致（`a67a8035…e6a42`）；新文案/切槽标记命中；`repack` RC=0。

## 4. 待办 / 下一步

1. 你确认是否执行方案 A（只读敲两条 avbctl 命令，可逆），我来指导并抓 log 验证。
2. 若 A 成立，再决定是否做方案 B（自动关 verity+verification）。
3. 刷入 `…2350.img` 验证完成页侧栏恢复。
4. reflash 失败提示改「请用 fastboot 刷 recovery_b」、AVB A/B/C 仍待拍板。

回退镜像：`…2346.img`（包名折行版）、`…2322.img`（切槽版）。
