# 撤销多余切槽 + 完成页补合并提示（含自动合并诊断）

日期：2026-11-17（跨零点）
镜像：`out/skyward-violin-OS4.0.5-20261117-0002.img`
SHA-256：`ddad31e9ad0e15b67eccc192ab3a7a86693ced13afb1cae6dfe98bbfa4566251`
本轮**未刷机、未写设备分区、未改 vbmeta**。

---

## 背景：用户实测纠正了上一轮的判断

用户手动"滑动以合并"后重启，**系统正常开机**。这证明：
- 卡 Mi 的根因不是"没切槽"，而是**虚拟 A/B 更新没完成合并**；
- 合并机制本身是好的（Skyward 的 `Merger` 能用）。

---

## 诊断 3：为什么自动合并没走通

### 确凿证据（只读）
- `/metadata/ota/state` 字段 `08 02` = `Unverified`（新系统已写入，等待启动确认）；
  无 `using_snapuserd` 字段 → 非压缩快照，Skyward 合并代码能处理。
- snapshots 里是 B 槽新系统快照（`system_b`/`product_b`/…）。
- 日志时间线：
  ```
  15:42:31  update_engine: SetActiveBootSlot: set slot 1 as active   ← 系统自己已切槽
  15:42:34  skyward: slot B starts next after the install            ← 我们上轮加的，多余
  ```
- Skyward reflash 只写 recovery 分区，**不碰** `/metadata/ota`，未干扰合并状态。
- 官方 `set_slot()` 的保护注释明确："更新进行中时，slot 由 update_engine 自己设，别的路径不该碰"。

### 推断（未获内核 panic 坐实，dmesg 被 recovery 启动冲掉、pstore 为空）
虚拟 A/B 正常链路："新槽首次启动成功 → 标记 boot successful → 系统内 update_engine 自动合并"。
violin 上新槽带**快照叠加视图**首次启动就卡 Mi，走不到"标记成功 → 自动合并"，于是状态永远
停在 `Unverified`（页面显示"更新等待合并"）。用户手动合并把快照写进真实分区后，系统读到完整
新系统即可启动——与此前怀疑的 verity 全开导致首次启动校验失败是同一条线索。

**结论**：不是"合并功能坏了"，是"新槽带快照首次启动失败，导致自动合并的前提不成立"。
属平台/AVB 层面问题；1+2 是防止再被误导，真正自动合并成败仍取决于 verity 那条线。

---

## 改动 1：撤销多余的自动切槽

`crates/skyward/src/ui.rs`：
- 删除 `activate_installed_slot()` 整个函数；
- 删除 `installer_told()` 里的 `new_slot` 计算和完成后的切槽调用块；
- 留注释说明：活动槽由 update_engine 设置（`SetActiveBootSlot`），此处不再重复。

日志里 `starts next after the install` / `could not make slot` 两个标记现为 0，确认清除。

---

## 改动 2：完成页补合并提示

`crates/skyward-ui/src/pages.rs` 的 `finished()` → `Outcome::Installed` 分支：
当 `cx.state.update` 为 `Installed(_) / Waiting / Compressed`（待合并）时，在"重启到系统"按钮下加一行：

> **简中**：更新尚未合并：新系统首次启动后会自动完成，请勿在启动完成前强制重启。若反复停在开机界面，请回到恢复模式，在「高级 > 系统更新」中手动合并后再重启。
> **繁中**：更新尚未合併：新系統首次啟動後會自動完成，請勿在啟動完成前強制重新啟動。若反覆停在開機畫面，請回到復原模式，在「進階 > 系統更新」中手動合併後再重新啟動。
> **English**：The update is not merged yet: the new system finishes it on its first boot, so don't force a restart before it comes up. If the phone keeps looping at the startup screen, return to recovery and merge it under Advanced > System update, then restart.

其它状态不显示这行；完成页仍保留 reflash 结果与"选择其他文件/再次发送"等按钮。
官方、dijun、onyx 的完成页原本都没有这句（只说"重启后将从该槽位启动"）——这是上游文案缺口，
在此补齐，避免用户把"卡在首次启动"误判为刷机失败而反复强刷。

---

## 验证

- Android release RC=0（1 m 02 s，无新增 warning）；宿主 pack RC=0；打包 RC=0。
- 镜像内二进制 SHA 与本地一致（`d37e13cf…9919b`）。
- 切槽标记 0（已撤销）；新合并提示、包名折行、5 分钟提示、真实进度、锁屏等标记均在。
- `mtp.rc` 存在、`system-init` 缺席、配置为 `mtp=false / mtp_after_decrypt=true / 90Hz / rotate=270 / columns=560`。
- `repack` RC=0。

## 待办 / 仍需拍板

1. **verity 验证**（上轮 A 方案）：在 recovery 里 `adb shell avbctl disable-verity` +
   `disable-verification`（只改当前槽、可逆），重启看新槽能否首次启动成功、自动合并走通。
2. 若确认 verity 导致，再做"安装后自动关两项 + 一键恢复"（对应最初 TWRP commit 意图）。
3. reflash 失败提示改「请用 fastboot 刷 recovery_b」。

回退镜像：`…20261116-2350.img`（`15ff95f5…f6a2`，切槽+侧栏版）、
`…20261116-2346.img`（`3fa793af…0cf5`，包名折行版）。
