# 按上游 PR#43 / Issue#67 修复：写回 EPERM + 卡 Mi 自动合并 + hold_required

日期：2026-11-17
镜像：`out/skyward-violin-OS4.0.5-20261117-2304.img`
SHA-256：`d17ccf679647a620400ec0430a4b967baa24a96aa788805d8579fc47b106ce2d`
本轮**未刷机、未写设备分区**。

依据：`.references` 调研报告 `docs/FINDINGS-upstream-pr-2026-11-17.md`（Issue #67、PR #43、PR #63）。

---

## 1. 写回 recovery 的 EPERM —— 移植 PR #43 的块设备只读标志处理

### 根因（上游确认）
OTA 校验器会把目标分区置为**块设备只读**（`BLKROSET`），普通 `write_all` 报 `Operation not permitted`。此前我们猜的"当前槽保护/时机/dd"都不是根因。

### 改动
- 新增 `crates/skyward-install/src/image/protection.rs`（约 75 行）：
  - `BLKROGET(0x125e)` 读标志；若为只读，`BLKROSET(0x125d)` 临时清 0；写完+读回校验后恢复为只读。
  - `ReadOnly` 带 `Drop` 守卫：任何失败路径都恢复原标志并写 kmsg。
  - 只对块设备发 ioctl，普通文件不受影响；ioctl 类型按 bionic/glibc 用 `libc::Ioctl` 转换。
- `image::flash()`：打开每个设备前 `ReadOnly::clear(path)`；写完且**关闭 fd 后**（设备不能带写 fd 恢复只读）逐个 `restore()`，聚合错误。
- `#[cfg(any(target_os="linux", target_os="android"))]` 隔离，宿主打包器不受影响。
- **撤销**上轮的 dd 式 `write_from_device()` + 4 次重试，`run.rs::reflash()` 回归简单的 `kept.write()`；删除 `REFLASH_TRIES/REFLASH_WAIT` 常量与 `keep.rs` 的 `source()/write_from_device()`。

### 标记
`clear the read-only flag` / `restore the read-only flag` 各命中 1；旧 `writing recovery back did not work yet` 为 0。

---

## 2. 卡 Mi：Unverified 自动合并 —— 对应 Issue #67

### 根因（Issue #67 官方确认）
虚拟 A/B 完整包写到未启动槽后，快照停在 `Unverified`（`state=08 02`）。Skyward 的 `standing()` 只对 `Merging` 起合并线程，`Unverified` 无人推进；Android init 也只验证不合并 → 切过去的槽卡开机画面。Issue 仍 open，官方未修。
（Issue #67 用官方同 commit 镜像、同样 HAL、dmesg 无 AVB 报错也复现——排除了我们此前猜的 verity / boot HAL 变体。）

### 改动
`crates/skyward/src/ui.rs::open()`：在读 `standing()` 处，原逻辑仅 `Merging` 续接；新增 `Standing::Waiting`（= Unverified 且当前槽是快照 target）时自动 `merge::Merger::start(slot, true)` 推进合并，写 kmsg
`the new system starts here with its update unmerged; merging it now`。
- 复用现有 merger 状态机：`merge_told()` follow 进度，完成后状态转 `Merged`。
- 落点在 open()（recovery 启动时）——正好覆盖"刷完包重启进 recovery"与"系统卡住后进 recovery"两种场景。
- 标记 `merging it now` 命中 1。

### 效果
刷完官方包后，若重启停在开机画面，用户再进 recovery 时会自动开始合并，合并完再重启即可正常进系统——不必手动去「系统更新」页滑「滑动以合并」。

---

## 3. hold_required：自动保留开关的安全语义（PR #43）

### 改动
- `keep.rs` 新增 `Kept::hold_required()`：需保留但当前槽 recovery 不是已安装的 Skyward 时，安装前明确拒绝并提示"Install Skyward first, or turn off 'Flash Skyward again'"。
- `run.rs::hold()` 改用 `hold_required()`：开关打开时必须有可保留的 Skyward 源，否则在 detach/写入任何分区之前就报错，不再静默跳过。
- 标记 `automatically flashing Skyward back needs` / `Flash Skyward again` 命中。

---

## 4. 不变的既有修复
进度条真实字节、禁用 AVB 按钮、包名折行、安装完成页合并提示、侧栏放行、真实进度 kmsg、锁屏右滑、双击亮屏、MTP 解密后启用、90Hz 等全部保留（镜像内标记核对通过）。

---

## 5. 验证

| 项 | 结果 |
|---|---|
| Android release 构建 | RC=0（1 m 10 s，无新增 warning） |
| 宿主 skyward-pack | RC=0 |
| 打包 | RC=0，104,857,600 字节（ramdisk 49,190,343） |
| 二进制一致性 | 镜像内与本地同为 `7fac2bb2…9ef81` |
| 新标记 | read-only clear/restore、merging it now、hold_required、Flash Skyward again 全命中 |
| 旧标记 | dd 重试、切槽标记均为 0 |
| 配置 | mtp=false / mtp_after_decrypt=true / 90Hz / rotate=270 / columns=560 |
| repack | RC=0 |

## 6. 刷入后验证清单

1. **写回 recovery**：刷官方包后底部不再提示"未能将 Skyward 重新刷入 recovery 分区"；日志 `this recovery written back to [B]: Ok(())`。
2. **卡 Mi 自动合并**：若重启后停在开机画面 → 再进 recovery，日志应出现 `the new system starts here with its update unmerged; merging it now`，随后自动合并；合并完成后重启正常进系统。
3. **hold_required**：若当前槽不是 Skyward 却打开自动保留开关，安装前应明确拒绝（而不是静默跳过）。
4. 既有功能无回归：进度条逐步走、禁用 AVB 按钮可用、包名折行、90Hz、触摸、WLAN。

回退镜像：`…20261117-0022.img`（dd 式写回版）、`…20261117-0015.img`（AVB 按钮版）。
