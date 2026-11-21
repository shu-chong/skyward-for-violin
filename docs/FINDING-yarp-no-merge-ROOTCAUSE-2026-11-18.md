# 卡 Mi 的真正根因（铁证）：update_engine 版本差异导致"直接写实体分区" vs "走快照"

日期：2026-11-18 23:52
抓证据方式：用户用 yarp 刷同一官方包 OS4.0.6.0，**刷完不重启**，趁还在 recovery 一次性抓取。
证据文件：`.references/logs/yarp3-recovery-after-20261118-235238.txt`（3,298 行，完整 `/tmp/recovery.log`）
对比镜像：`.references/img-compare/`（yarp-ues.bin / skyward-ues.bin / 两边 ramdisk 清单）

## 结论（推翻此前所有推断）

**yarp 刷完能直接开机，是因为它的 `update_engine_sideload` 在创建快照失败后，
回退成"直接写实体分区"（no-snapshot sideload）——不留快照、state 空、重启即进系统。
Skyward 的 `update_engine_sideload` 是另一个版本，成功走完了虚拟 A/B 快照路径，
留下 Unverified（state=08 02）+ 10 个快照，所以必须先进 recovery 合并一次才能开机。

差异不在 recovery 本身、不在 boot HAL、不在 snapuserd、不在 verity、不在安装命令——
就在两边 ramdisk 里那个 `system/bin/update_engine_sideload` 二进制的版本/行为。**

## 决定性日志（yarp 这次，逐字）

刷前基线：a 槽、state 空、snapshots=0、`/tmp/recovery.log` 2197 行。
刷后（未重启，仍在 recovery）：a 槽、state 空、snapshots=0、snapuserd 未运行。

同一份 log 里 update_engine 自己的完整轨迹：

```
23:47:10  InstallPlan: source_slot: A  target_slot: B
          switch_slot_on_reboot: true   run_post_install: true   write_verity: true
          size: 8770311080  metadata_size: 299347

          Using AIDL version of IBootControl          ← boot HAL：AIDL recovery 版（和 Skyward 相同）
          SnapshotManager initialized.
          Successfully created snapshot partition for odm_b / product_b / system_b / vendor_b ...  ← 先创建快照
          Allocating CoW images.
          ERROR: Cannot create /data backed snapshots in recovery.   ← 关键①
          ERROR: CreateCowImage failed: No space left on device
          Calculated needed COW space: 9912389632 bytes               ← 需要 9.9 GB COW
          ERROR: Cannot create update snapshots: No space left on device
          PrepareSnapshotPartitionsForUpdate failed in recovery. Attempt to overwrite existing partitions if possible  ← 关键②
          Cancelling update from state: 1 → Removing all update state  ← 关键③：清掉快照状态
          Not using snapshot on VAB device because sideloading.        ← 关键④：改用非快照写入
          Added partition system_b/product_b/vendor_b ... to group xring_dynamic_partitions_b
          [逐分区直接写实体逻辑分区]

23:51:27  Verity writes disabled on partition vbmeta / system / vendor ...（write_verity）
          Unmapped all partitions → Update successfully applied, waiting to reboot.
          Writing update complete marker, slot 0
          I:Command=dd bs=1048576 if=recovery_a of=recovery_b   ← TWRP 自动写回 recovery
          I:operation_start: 'Disable AVB2.0' → 处理 vbmeta_a / vbmeta_b 成功   ← 用户手动执行
          I:operation_start: 'Flash Image' → 刷 kernelsu patched init_boot 到 a/b 两槽   ← 用户手动执行
```

**这一串解释了全部现象**：yarp 尝试虚拟 A/B 快照 → recovery 里无法创建 data-backed COW
（需要 9.9 GB，不够）→ **主动取消快照状态、回退到直接覆盖实体分区** → 所以刷后 state 空、
snapshots=0、没有 Unverified → 重启直接进系统。

## 两个 update_engine_sideload 的二进制差异（镜像实证）

| 关键字符串 | YARP (3,115,392 B) | Skyward (2,975,424 B) |
|---|---|---|
| `Cannot create /data backed snapshots in recovery.` | ✅ 有 | ❌ **没有** |
| `Not using snapshot on VAB device because sideloading.` | ✅ | ✅ |
| `Attempt to overwrite existing partitions if possible` | ✅ | ✅ |
| `Allocating CoW images` | ✅ | ✅ |

两边都有"回退"相关字符串，但**只有 YARP 版会在 recovery 里真的走到回退**——
因为它带 `Cannot create /data backed snapshots in recovery` 这条检查/报错逻辑，
在 COW 空间不足（本例需 9.9 GB、super 里只有 305 MB）时触发 `RemoveUpdateState` + `Not using snapshot`。
Skyward 版没这个分支，于是**快照创建成功、走完整虚拟 A/B 路径**，留下 Unverified 待合并。

> 注：Skyward 刷机那次的 UES 日志被重启冲掉，没拿到它逐行的 snapshot 轨迹，
> 但它的**结果**（state=08 02 + 10 个 system_b/product_b/… 快照）只能由"快照路径走通"产生，
> 与 yarp 这次"回退直接写、state 空"形成完整对照。

## 推翻此前所有推断（诚实记录）

| 此前说法 | 本次铁证结论 |
|---|---|
| "yarp 有 snapuserd 能就地合并，Skyward 没有所以卡" | ❌ **错**。yarp 这次 snapuserd 根本没运行、mode 文件不存在、snapshots 从没留下——它压根没走合并，是没产生快照 |
| "boot HAL 变体不同" | ❌ 错。yarp 日志明写 `Using AIDL version of IBootControl`，两边同一个 recovery 版 HAL |
| "verity / verification 全开导致" | ❌ 非主因。yarp 走的是 write_verity=true 后禁用，且它根本没快照要校验 |
| "没切槽" | ❌ 错。`switch_slot_on_reboot: true`，update_engine 自己切 |
| "安装方式（img vs zip）不同" | ❌ 错。同一个 zip、同一个 payload |
| ✅ **真正根因** | **UES 版本差异 → 是否回退成非快照直写 → 是否留下 Unverified 待合并** |

## 修复方向（下一步，等用户拍板）

要让 Skyward 也"刷完直接开机、不卡 Mi"，最直接的办法：
**把 yarp 的 `system/bin/update_engine_sideload`（3,115,392 B，带 data-backed-COW 回退逻辑）
换进 Skyward 镜像，替换现在从 stock 底稿带的旧版（2,975,424 B）。**

这样 Skyward 刷官方包时会和 yarp 一样：快照创建失败 → 回退直接写实体分区 → 不留快照 →
重启直接进系统，不再需要"卡一次 → 进 recovery 合并"。

风险/前提：
- 该 UES 来自 yarp 镜像（系统组件），替换需确认与 Skyward ramdisk 的依赖库兼容
  （libsnapshot/liblp/libfstab 等）——两者都来自同一台机器的 stock，很可能兼容；
- 需确认替换后 "安装官方包写回 recovery"（PR#43 protection）和"hold_required"仍正常；
- 备份以便回退（旧 UES 已存 `.references/img-compare/skyward-ues.bin`）。

备选（保守）：保留现状"卡一次 → 进 recovery 自动合并"（已真机跑通），不换 UES。

## 补充：不需要下载其它机器的 Skyward（本地三方对比已证）

用户问"是否要下载别的机器 skyward 解压看怎么处理"。答案：**不用**，本地就有决定性样本：

| UES 来源 | 大小 | SHA-256 前缀 | 含 `Cannot create /data backed snapshots in recovery` |
|---|---|---|---|
| OS4.0.5 官方 recovery（Skyward 打包底稿） | 2,975,424 | `609f3c21…` | ❌ |
| **OS4.0.6.0 官方 recovery**（`D:\DNA_WIN-230828\...OS4.0.6.0...\001_DNA\recovery.img`） | 2,975,424 | `609f3c21…` | ❌ |
| 当前 Skyward 镜像 | 2,975,424 | `609f3c21…` | ❌ |
| **YARP 镜像（唯一带回退逻辑的）** | 3,115,392 | `cde4ea4a…` | ✅ |

结论：
- OS4.0.6.0 官方 recovery 的 UES 和 Skyward 用的**字节完全相同**（同一 SHA）——即官方 ROM 自带的就是旧版；
- 三版 UES 都有"回退"通用字符串（`Not using snapshot…`、`Attempt to overwrite…`），
  **唯独 YARP 版多了 `Cannot create /data backed snapshots in recovery` 这条检查**，
  正是它在 COW 空间不足时触发了 `RemoveUpdateState` + 回退直写；
- 所以"别的机器怎么处理"这个问题不需要跨机型下载：**唯一已知能回退的样本就是 YARP 版 UES，
  而且它已经在我们手里**（`.references/img-compare/yarp-ues.bin`）。当然，这也意味着
  YARP 版 UES 是 yarp 项目自己带的新版，不是小米 stock 的——换进去属"借用第三方系统组件"，
  需在真机回归验证（解密/刷机/写回 recovery）。

## 重要修正（23:59，dijun 对照后）："换 UES"方案作废，根因回到 COW 空间

用户提出：dijun（同玄戒 O1 平台）用户反馈刷完包**没有卡 Mi 再合并**。我据此查了 dijun 实证：

| 样本 | UES SHA 前缀 | 含 `Cannot create /data backed snapshots in recovery` | 刷官方包卡 Mi？ |
|---|---|---|---|
| dijun v0.2.1 官方镜像（本地 `.references/dijun-release.img`） | `609f3c21…` | ❌（旧版，和我们**字节相同**） | 用户反馈：不卡 |
| violin OS4.0.5/4.0.6 官方 + 我们 Skyward | `609f3c21…` | ❌ | 卡（实测） |
| yarp | `cde4ea4a…` | ✅（新版，会回退直写） | 不卡（实测） |

**这推翻了"换 YARP 版 UES 就能解决"的方案**：dijun 用的是和我们**完全相同的旧 UES**，
却不需要卡 Mi。所以"回退直写"不是唯一解，旧 UES 在 dijun 上也能顺畅通关。

那 dijun 为什么不卡？从源码和日志层面能确定的：
1. dijun 的 `open()` 和官方基线一样，**只在 `Standing::Merging` 续合并，没有 `Waiting` 自动合并**
   （我们 2304 版那句 `merging it now` 是 dijun 没有的）；
2. dijun 的 update_engine 参数和我们**完全相同**（4 个参数）；
3. dijun 的 `prop.default` 里 `ro.virtual_ab.*`、`ab_ota_partitions` 和我们**完全相同**；
4. 唯一实质性差别：**dijun 原厂是 OS4.0.4.0（Android 16），violin 是 OS4.0.5/4.0.6（Android 17）**，
   以及 super 分区布局/COW 余量不同。

结合 yarp 这次日志里 `Remaining free space for COW: 305135616 bytes`、`Calculated needed COW space: 9912389632 bytes`
——violin 的 super 里只剩 305 MB COW，而要建的快照需 9.9 GB。**这才是 violin 特有的约束**：
- yarp 用新版 UES：检测到 COW 不足 → 回退直写 → 不卡；
- Skyward 用旧 UES：COW 不足时依旧建出快照（留给系统合并）→ 玄戒/HyperOS 系统侧不推进 → 卡；
- dijun 用旧 UES 却不卡：**最可能是 dijun 的 super 有足够 COW 建快照，且它的系统首启能自动合并**
  （Android 16 vs 17、不同 ROM 行为），因此旧 UES 走快照路径也能顺畅通关。

**结论修正**：卡 Mi 是 **"violin 的 COW 空间不足 + 旧 UES 不回退 + 玄戒 HyperOS 系统侧不推进合并"**
三者叠加的结果，不是单一 UES 版本问题。换 UES 属于"用 yarp 的回退掩盖 COW 不足"，
能解 violin 症状但是绕路，且借用了第三方组件；治本要么补 snapuserd/自动合并（我们已做半自动），
要么让系统侧正常完成合并。dijun 不卡是因为它的环境不具备这三个叠加条件。

> 诚实边界：dijun"为什么不卡"的最后一环（是 COW 够、还是系统自动合并）我**没有 dijun 真机日志**，
> 无法 100% 坐实，只能列出已排除项和最可能原因。若需坐实，需 dijun 用户刷包时抓一次同样的 recovery.log。

### v0.2.2 镜像终检（用户 00:05 提供的 `C:\Users\shc\Desktop\skyward\skyward-v0.2.2-dijun-OS4.0.4.0.XODCNXM.img`）

把 v0.2.2 镜像解出关键组件再验一遍，结论不变：
- UES：`609f3c21…`（与我们/官方**字节相同**），无回退检查；
- `prop.default` 的 `ro.virtual_ab.*` 与我们完全相同；
- recovery 主程序 10.6 MB（比我们 8.8 MB 大，是 dijun 的蓝牙/音频等功能），但
  **没有** `the new system starts here with its update unmerged` / `merging it now`
  这两句——即 **v0.2.2 连我们 2304 版的 Waiting 自动合并都没有**，代码层面比我们更"素"；
- 无 snapuserd。

**所以 v0.2.2 不卡 Mi 更坐实了"与 recovery 代码/UES 无关"**：它比我们还少的机制却不需要卡。
唯一剩下的解释就是设备环境差异——dijun 原厂 OS4.0.4.0 / Android 16、super 分区 COW 布局与
violin OS4.0.5/4.0.6 / Android 17 不同（issue #67 证明高通 OS4.0.6 也卡，进一步指向
"OS4.0.6 的更新方式/COW 需求"而非平台）。这仍属推断，除非拿到 dijun 刷包现场日志。

### 4 台官方 v0.3.1 镜像横向验证（用户 00:11 提供，全部位于 `C:\Users\shc\Desktop\skyward\`）

用户给了 4 台机器的官方 Skyward v0.3.1 镜像，用来验证"是不是所有机器都靠某种特殊机制不卡 Mi"。
提取各自的 UES / prop / recovery 主程序：

| 镜像 | Android (prop) | UES 大小 | UES SHA 前缀 | 回退检查 | snapuserd | recovery 里 Waiting 自动合并 |
|---|---|---|---|---|---|---|
| annibale-OS3.0.307.0.WPKCNXM | 15 | 3,055,936 | `3f7c442e…` | ❌ | ❌ | ❌ |
| byron-OS3.0.310.0.WAFCNXM | 16 | 3,103,936 | `9fcd2881…` | ❌ | ❌ | ❌ |
| popsicle-OS4.0.4.0.XPBCNXM | 16 | 3,103,936 | `e82180a3…` | ❌ | ❌ | ❌ |
| myron-OS4.0.4.0.XPMCNXM | 16 | 3,103,936 | `e82180a3…`（与 popsicle 同） | ❌ | ❌ | ❌ |
| dijun v0.2.2（玄戒，Android 16） | 16 | 2,975,424 | `609f3c21…` | ❌ | ❌ | ❌ |
| 我们 Skyward 2304（violin） | 17 | 2,975,424 | `609f3c21…` | ❌ | ❌ | ✅（我们加的） |
| yarp（violin） | — | 3,115,392 | `cde4ea4a…` | ✅ 唯一 | ❌ | — |

**这张表说明三件事**：
1. **所有官方 v0.3.1 镜像（高通 Android 15/16、玄戒 Android 16）都没有那条回退检查、
   都没有 snapuserd、都没有 Waiting 自动合并**——即官方设计里压根没有"免卡 Mi 合并"的特殊机制；
   它们不卡，是因为它们的系统版本（OS3.x / OS4.0.4，Android 15/16）本身不产生那个待合并陷阱；
2. 唯一的例外是 **yarp 的第三方新版 UES**（带回退检查）；
3. 我们 2304 版的 Waiting 自动合并在所有官方镜像里都**不存在**，是我们为 violin OS4.0.6 专门补的兜底。

结合上游 issue #67（annibale/K90 高通 **OS4.0.6.0** 也卡 Mi），可以给出最终结论：
**卡 Mi 与平台（高通/玄戒）、与 Skyward 代码/UES 都无关，是 OS4.0.6 这一代 Android 17 更新的
"Unverified 无人推进合并"通病**。所有停在 OS3.x/OS4.0.4（Android 15/16）的机器都不踩坑；
只有升到 OS4.0.6 的机器（violin、K90 annibale）才踩。官方 issue #67 至今 open、未修。

## 附：本次刷机后续动作（用户已澄清：后两项是手动做的）

按日志顺序，刷官方包这一串操作里：
1. `dd recovery_a → recovery_b`（写回 recovery）—— TWRP 刷 A/B ZIP 后的自动动作；
2. `Disable AVB2.0`（vbmeta_a / vbmeta_b 都处理成功）—— **用户手动点击执行**；
3. `Flash Image`：刷 `kernelsu_patched_20261116_044735.img` 到 init_boot_a / init_boot_b —— **用户手动执行**。

（用户 2026-11-18 23:54 明确澄清：AVB2.0 和 Flash Image 这两项是他手动刷的，
不是 yarp 自动后处理。此前"可能是用户配置的 TWRP 后处理"的措辞不准确，在此更正。）

**这一点反而强化了根因判断**：`/metadata/ota/state` 变空、snapshots=0、以及
`Not using snapshot on VAB device because sideloading`（回退成直接写实体分区）
全部发生在 **Flashing（update_engine）阶段、23:51:28 就已成立**——
远早于用户手动做的 AVB 禁用和 KernelSU init_boot。所以"刷完不卡 Mi"的原因
与这两项手动操作无因果关系，纯粹是 update_engine 走了非快照直写路径。

本轮纯只读抓取，**未刷机、未写设备分区**（刷机与后续手动操作均由用户执行、这次特意不重启）。
