# 高级页新增「禁用 AVB 验证」按钮（无恢复按钮）

日期：2026-11-17
镜像：`out/skyward-violin-OS4.0.5-20261117-0015.img`
SHA-256：`1db517a4c6fbf183949fdd63299c7132d1940553fef8b04180956cd609671845`
本轮**未刷机、未写设备分区**；vbmeta 的实测读取是只读的。

---

## 背景：yarp 的「禁用 AVB2.0」实测

用户在 yarp 点击后，从设备只读读出：
- `vbmeta_a/b` 的 flags（偏移 120）= `00 00 00 01`（bit0 置位）；
- `avbctl get-verity` = disabled、`get-verification` = enabled。

原理：vbmeta 头偏移 120 的 4 字节 flags 里，
- **bit0 = HASHTREE_DISABLED**（dm-verity，运行时逐块校验）
- **bit1 = VERIFICATION_DISABLED**（bootloader 启动校验）

TWRP commit `15bb3f70` 只是让"禁用 AVB"同时清 bit0+bit1 并写 `vbmeta_a/b`，不改其它字节。
Skyward 的 `avbctl` 用**完全相同的偏移和位定义**（`FLAGS_AT=120`、`Check::off()`=1<<0 / 1<<1），
底层能力早就具备，只缺 UI 入口。用户明确要求：**只要禁用按钮，不要恢复按钮。**

---

## 实现

### 底层（`crates/skyward-install/src/commands/avbctl.rs`）
新增公共函数 `disable_both()`：
- 检查 bootloader 是否 locked（locked 直接拒绝，与 avbctl 一致）；
- 一次读取当前槽顶层 vbmeta 头（256 字节），在 `FLAGS_AT=120` 处同时置 bit0+bit1，
  **一次写回**（不产生只关一项的中间状态），`sync_all()`；
- 返回给用户的提示文本；由 `commands.rs` 以 `disable_avb` 导出。
- 无独立 vbmeta 分区时复用 `top_level()` 从 boot 分区 footer 定位 vbmeta，与原 avbctl 一致。

### UI（`skyward-ui`）
- `Screen::Avb`、`Confirm::DisableAvb`、`Effect::DisableAvb` 三个新枚举项；
- 高级页新增一行「禁用 AVB 验证」（红色盾牌图标），打开子页；
- 子页 `avb()`：说明"关闭 dm-verity + bootloader 校验，卡开机时通常可正常启动"，
  警示"仅对 unlocked bootloader 有效、**本页不提供恢复，恢复需重刷官方 vbmeta**、重启生效"，
  底部滑动二次确认「滑动以禁用」（danger）；
- `confirm_label` 登记新文案；`render` 路由登记；`search.rs` 登记搜索入口（avb/vbmeta/verity 等关键词）。

### 运行时（`crates/skyward/src/ui.rs`）
`Effect::DisableAvb` 调用 `skyward_install::commands::disable_avb()`，结果用 `app.report()`
显示在页面；写 kmsg `disabling AVB checks, as the Advanced page asked`。失败只在页面提示，
**不中断 recovery**（写启动关键分区的失败也不让整个 recovery 退出）。

**没有恢复按钮**：与用户要求一致。要恢复需重刷官方 vbmeta（或 adb 手动 enable）。

---

## 验证

- Android release RC=0（1 m 03 s，无新增 warning）；宿主 pack RC=0；打包 RC=0。
- 镜像内二进制 SHA 与本地一致（`12d1d003…ea2`）。
- 新文案/标记全部命中：`禁用 AVB 验证`、`滑动以禁用`、`关闭当前启动槽位 vbmeta`、
  `disabling AVB checks`、`verity and verification are disabled`、`本页不提供恢复`。
- 上轮撤销的切槽标记仍为 0；镜像内 `system/bin/avbctl` 仍在（底层命令未破坏）。
- `repack` RC=0。

## 刷入后验证清单

1. 高级页出现「禁用 AVB 验证」，点开有说明 + 警示 + 滑动确认。
2. 滑动后页面提示 `verity and verification are disabled on slot with suffix _b`。
3. 只读确认 flags：`dd if=/dev/block/by-name/vbmeta_b bs=1 skip=120 count=4 | od -An -tx1`
   应为 `00 00 00 03`（bit0+bit1 都清）。
4. 重启，看新槽能否首次启动成功、自动合并走通。
5. 确认没有"恢复"按钮（符合要求）。

回退镜像：`…20261117-0002.img`（撤销切槽 + 合并提示版）、
`…20261116-2350.img`（切槽+侧栏版）。
