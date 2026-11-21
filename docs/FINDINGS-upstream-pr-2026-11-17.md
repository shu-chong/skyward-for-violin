# 上游 PR/Issue 调研：刷完官方包不开机、写回 recovery EPERM 的官方解法

日期：2026-11-17
范围：`kirklin/skyward`（remote origin）全部 PR（open+merged+closed，共 40+）、相关 issue 搜索
方法：GitHub REST API（`pulls?state=all`、`pulls/{n}/files`、`search/issues`），纯只读

---

## 结论速览

| 我们的现象 | 上游对应 | 官方根因/解法 | 我们之前做的 | 是否要做对 |
|---|---|---|---|---|
| 刷完官方包切槽后卡 Mi、停在 `Unverified` | **Issue #67**（annibale，同 Android17/OS4.0.6/虚拟AB，**现象一字不差**） | Skyward 0.3.1 设计缺口：`standing()` 只对 `Merging` 起合并线程，`Unverified` 无人推进；init 也只验证不合并 | 先猜"没切槽"→切槽（已撤销）；再猜"verity"→AVB 按钮；再猜"boot HAL 变体" | **都不是根因**，#67 已明确是无主线程推进 `Unverified` |
| 写回 recovery 报 `Operation not permitted` | **PR #43**（socrates，未合并）新增 `image/protection.rs` | **OTA 校验器把目标分区设为 read-only（BLK ioctl BLKROSET）**；flash 前 `BLKROGET` 读标志、临时 `BLKROSET` 清 0、写完+校验后恢复（Drop 守卫） | dd 式写回 + 重试（`…0022.img`） | **方向不对**，应移植 protection.rs |
| "自动刷入 skyward 开关" | PR #43 = `Options.keep`（UI 文案 "Flash Skyward again"） | 开关打开时 `Kept::hold_required()`：当前槽没有已安装 Skyward 就**安装前明确拒绝**，不静默跳过 | 我们的 `hold()` 返回 None 就静默跳过 | 应改为 hold_required 语义 |
| 进度条 1 秒 100% | **PR #63**（open） | 同源：FUSE 把读到的最远字节当进度 | 我们自己用 `bytes downloaded` 修了 | 我们的修法等价、可用 |

---

## 1. Issue #67 —— 卡 Mi 的精确官方描述（最重要）

标题：【annibale】整包装到未启动槽后切槽，快照停在 Unverified，没有一方推进合并，卡在开机画面
设备：REDMI K90（annibale，SM8750/sun，Android 17），OS4.0.6.0.XPKCNXM，Skyward 0.3.1（c5e09f6）

现场日志（与我们的几乎逐字相同）：
```
install: ... OS4.0.6.0 ... 9422753169 bytes
install: an update to 17OS4.0.261026.215942994.QCPECN.S
install: this recovery written back to [B]: Ok(())       ← 注意：他们写回成功
install: Installed { slot: B, wipe: false }
```
- 重启切到 B → 卡开机画面
- `/metadata/ota/state` = 80 字节、`08 02`（protobuf `state=2` = `UpdateState::Unverified`）
- `/metadata/ota/snapshots/` 九个 B 槽分区元数据齐全（system_b/vendor_b/product_b/… 我们也是 10 个）
- **进 recovery「双系统」页手动合并一次** → state 清零、mapper 40+→9、再重启正常开机

已排除（与我们一样）：两槽 boot/init_boot/vendor_boot/dtbo/vbmeta 哈希一致、pstore 空、dmesg 无 AVB/dm-verity/snapshot 报错、换别人编译的同 commit 镜像照样复现。

**issue 作者给出的根因**（与代码核对一致）：
> `state=Unverified` 意为"已写完、待验证/合并"。Skyward 的 `standing()` 只对 `Merging` 起合并线程，`Unverified` 落到别的分支不推进；Android init 在此状态也只验证、不主动把 cow 并回 base。两边都不动手 → state 永远停在 Unverified → 切过去的槽起不来。

**这纠正了我们此前的多个推断**：
- 不是"没切槽"（他们切了，update_engine 也确实切了）
- 不是 verity/verification（dmesg 无相关报错）
- 不是 boot HAL 变体（我们的 `.recovery` 推断缺乏证据）
- 也不是"系统镜像刷坏"（合并后同一系统正常启动）
- 是**虚拟 A/B 更新在 Skyward 里缺少 Unverified→Merging 的推进者**

**上游状态**：issue 仍 open，无评论、无关联 PR（timeline 无 cross-ref）。说明**截至 0.3.1 官方尚未修复**；"别人能直接开机"多半是：①用的是另一种安装方式（recovery 内直接 unpack/刷分区，不走 update_engine 的虚拟 A/B 快照）；②或其设备/ROM 的系统侧会自动完成合并。

---

## 2. PR #43 —— 写回 recovery EPERM 的真正根因与官方解法（未合并）

标题：适配 REDMI K60 Pro（socrates）：REC 解密版本兼容与刷包后保留（47 文件，+3023/-199，未合并）
实测：Socrates 用户从 Skyward 界面刷 ROM，`Skyward 自动回刷日志为 [A]: Ok(())`，分区 SHA 与 CI 一致，重启正常进系统。

### 2.1 EPERM 根因
新增文件注释原文：
> "The OTA verifier can leave its targets read-only."

即 **OTA 校验器把目标分区置为内核只读标志（block device read-only）**，导致 `write_all` 返回 EPERM——这与"当前槽保护""时机没卸载"都不同，是**块设备只读位**。

### 2.2 官方解法 `crates/skyward-install/src/image/protection.rs`
```rust
const BLKROSET: u32 = 0x125d;
const BLKROGET: u32 = 0x125e;
// flash() 对每个设备：
let mut flags = Vec::new();
for path in devices {
    flags.push(ReadOnly::clear(path)?);          // read_only? → BLKROSET(0) 临时可写
    let mut f = OpenOptions::new().read(true).write(true).open(path)?;
    ...
}
... 写 + 读回校验 ...
drop(outs);
for flag in &mut flags { flag.restore()?; }        // BLKROSET(1) 恢复只读
```
`ReadOnly` 带 `Drop` 守卫：任何失败路径都会恢复原只读标志并写 kmsg。只对块设备发 ioctl，普通文件不受影响；`#[cfg(target_os = "linux"/"android")]` 隔离。

### 2.3 配套：`hold_required`（即用户说的"自动刷入开关"）
`Kept::hold_required(dir, current)`：开关打开但当前槽 recovery 不是已安装的 Skyward 时，**安装前就明确报错**（"Install Skyward first, or turn off 'Flash Skyward again'"），不再静默跳过。同时新增 `examples/keep-recovery.rs`：安装后可用 `--write <源SHA256>` 单独重试写回，且要求 update_engine 已退出、源镜像摘要匹配。

---

## 3. PR #63 —— 我们进度条修复的官方版

标题：fix(install): count the payload's bytes as the update's progress（open）
根因原文：进度取"任意读取到达的最远字节"，读 zip central directory 后会跳到整包大小 → 100% 提前。
官方解法：sideload mount 在包已知后（`payload_offset`/`payload_size`）只盯 payload 区间，`apply` 只报该区间读到的字节，调用方 clamp 到 payload size。

我们的修法（ota.rs 解析 update_engine 日志 `bytes downloaded` + run.rs 用真值）**思路等价、可用**，无需替换；但官方 PR 在 FUSE 层解决，更通用。

---

## 4. 对我们此前判断的修正（诚实记录）

| 我们之前的推断 | 现在看 | 依据 |
|---|---|---|
| 卡 Mi 是"没自动切槽" → 加 set_active | 错，已撤销 | #67：切了槽照样卡；update_engine 自己也 SetActiveBootSlot |
| 卡 Mi 是 verity/verification 全开 → 做 AVB 按钮 | 无证据，#67 明确 dmesg 无 AVB/verity 报错 | #67 已排除 |
| 卡 Mi 是 boot HAL 用错变体（.recovery vs 普通） | 证据不足，已不再作为首要根因 | #67 用官方同 commit 镜像、同样 HAL 也复现，说明不是 HAL 变体 |
| EPERM 是 dd/时机/当前槽保护 → dd 式 + 重试 | 方向不对，真因是块设备只读位 | PR #43 protection.rs 明确 |
| 手动合并能开机 = 合并功能好、只是自动没触发 | 基本对，且 #67 说明这是 Skyward 0.3.1 已知缺口，无人推进 Unverified | #67 代码分析 |

---

## 5. 建议的下一步（按投入产出排序，等用户拍板）

1. **移植 PR #43 的 `protection.rs`**（约 70 行 + image.rs 十几行），替代我们上轮的 dd 式+重试——这是写回 EPERM 的官方正解，且能解释为什么 Socrates 用户 `[A]: Ok(())` 成功。
2. **针对 #67 做 Unverified 自动推进**：在 `decrypt_told()`/安装收尾检测到 `Standing::Waiting`（=Unverified 且当前槽是 target）时，自动起 merge（复用现有 `Effect::Merge`），或在完成页给醒目的"合并后再重启"入口（我们已加提示，但没自动）。这能让"刷完直接重启就能开机"，对齐用户说的别人设备行为。
3. `hold_required` 语义：开关打开但没 Skyward 源时安装前拒绝，避免静默留下别的 recovery。
4. 更新我们上轮的 `…0022.img`（dd 式）：其思路非官方解法，建议被 1 取代。

---

## 6. 附注：为什么"别人不用合并"

从 #67 的存在可推断，"刷完直接重启进系统"不是 Skyward 0.3.1 + update_engine 标准虚拟 A/B 路径的普遍行为，而是取决于**安装方式**：
- 若用 recovery 内的"刷入分区镜像/自安装 zip（updater-script 直接写 system 等分区）"，不走 update_engine 的快照机制，就没有 Unverified 待合并状态 → 刷完直接重启能进系统；
- 若用**完整 OTA zip 走 update_engine**（我们的方式），必然产生快照，必须有推进合并的一方；Skyward 0.3.1 缺这一环（#67）。
所以当别人说"有自动刷入 skyward 开关、刷完直接开机"，很可能他们的包或安装路径不同，或系统侧会自动合并——不能据此认定我们的切槽/HAL/verity 有问题。

（本报告纯只读调研：未改代码、未构建、未刷机。）
