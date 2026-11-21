# 刷完官方包必卡 Mi 的代码级复查（含注释勘误）

日期：2026-11-19
范围：`skyward-install`（装包）→ `skyward-vab`（快照/合并）→ `skyward/src/ui.rs`（装完的重启与合并）
方法：读当前工作区代码 + 复盘 `live-final-20261119-092521.txt`、`sysmerge-*`、`.references/logs/yarp3-recovery-after-20261118-235238.txt`、`.references/img-compare/`

---

## 0. 一句话结论

**卡 Mi 不是"没切槽"、不是"没写回 recovery"，也不是"我们少调了一次合并"。**
是我们**让 update_engine 把更新写成了"COW 镜像放在 /data 上"的形态**（super 里没有余量），
而新槽位首启时，first-stage init 必须先打开 `/data` 上的 COW 镜像才能挂 system —— 那时 `/data` 还没解密。
于是新系统起不来，只能等 COW 被合并回基础分区（Skyward 在 recovery 里做的事）之后才起得来。

我们的代码在这条链上有一处**明确的设计缺口**：安装成功后**既没有合并，也没有检查新槽位是否可启动**，
页面上却只说"安装成功"（`Work::Update` 这类由 misc 发起的更新甚至会 1.5 秒后自动重启，
`ui.rs:2065-2066`）。用户看到的"成功"其实是一个起不来的槽位。所以"必然卡一次"是代码决定的，不是偶发。

---

## 1. 事实链（可复核）

| 时间 | 事件 | 证据 |
|---|---|---|
| 01:23:15 | Skyward 起在 **b** 槽（`mounted system_b`…） | live-final 3709-3718 |
| 01:25:28 | 从 `/sdcard/Documents/violin-ota_full-OS4.0.6.0…zip` 开始装（8,770,320,010 B） | live-final 17910 |
| 01:25:34 | update_engine 在 **`/data/gsi/ota/`** 创建 `system_a-cow-img.img`、`vendor_a-cow-img.img`… 共 10 个 COW 镜像（dev=`dm-10`） | live-final 18516-18674 的 SELinux 记录 |
| 01:31:26 | `SetActiveBootSlot: set slot 0 as active`（切 a，UES 自己切的） | live-final 39022 |
| 01:31:30 | 写回 recovery A 成功；`Installed { slot: A }` | live-final 39637-39638 |
| 01:31:31 | 完成页；从页面刷包时不会自动重启（`leaves` 只对 `Work::Update` 为真） | live-final 39638；`ui.rs:2049-2066` |
| 01:32:20-01:32:33 | 用户手动刷 KernelSU `init_boot`、`vendor_boot` 到 a/b | live-final 42673-43562 |
| 之后 | 手动重启 → 卡 Mi，反复重启 | 用户实测 |
| 01:37:59 | 手动进 Skyward（此时在 a 槽）；`/metadata/ota/snapshots/` 下 10 个快照，`state` 仍是 Unverified（`08 02`） | sysmerge 3767-3776、7162-7164；state 字节见 0935 轮抓取 |
| 01:38:05 | 我们自己的兜底：`merging it now` | sysmerge 6009 |
| 之后 | 合并完成，重启进系统 | 用户实测 |

对照（yarp 刷同一个包）：

- super 里可用的 COW 只剩 **305,135,616 B（约 291 MiB）**，而这个包**需要 9,912,389,632 B（约 9.2 GiB）**；
- 新版 UES 直接拒绝：`Cannot create /data backed snapshots in recovery.` →
  `Cancelling update from state: 1` → `Not using snapshot on VAB device because sideloading.` →
  `Attempt to overwrite existing partitions if possible` → **直写实体分区，state 空，重启即进系统**。

这两条日志是同一个包、同一台机器、同一个 `/metadata` 布局，唯一差别就是 **UES 版本**（yarp 多那条"recovery 里不许建 /data 快照"的检查）。

---

## 2. 为什么"COW 在 /data"就一定起不来

1. 虚拟 A/B 有两种 COW 落点：
   - **super 里的 `*-cow` 分区**（内核 dm-snapshot 直接读，first-stage init 自己就能映射）；
   - **`/data/gsi/ota/*-cow-img.img` 文件镜像**（libfiemap 记录在 `/metadata/gsi/ota/lp_metadata`，靠 `dm-linear` 指到文件的物理区间）。
2. 这次是后者：`cow_partition_size==0`、`cow_file_size>0`，并且 `using_snapuserd=false`。
   最后这一点**不是推断**：若 `using_snapuserd` 为真，`standing()` 会返回 `Compressed`（`vab/lib.rs:379`），
   `begin()` 的 `Waiting` 分支根本不会触发（`ui.rs:1600`）；而 01:38:05 实际打出了 `merging it now`，
   并且 `snapshots::map_snapshot` 的 "compressed and served by snapuserd" 保护也没拦住
   （`snapshots.rs:470-476`）。三处互证：这次是**内核快照 + /data 文件 COW**。
3. 内核快照形态要在 **first stage** 就把 `system_a` 的 dm 表建起来。建表前 libsnapshot 必须
   `MapCowImage()`：打开 `/data/gsi/ota/*.img` 校验、再按 `lp_metadata` 里的区间建 dm-linear。
   **first stage 打不开 /data**（FBE，密钥在 second stage 的 vold/keymint 手里）。
4. 于是 init 在挂 system 前失败 → 内核已经起来（Mi logo 是 bootloader 的），Android 没起来 → 重启，循环。
5. 合并（Skyward 做的）把 COW 写回 `system_a` 等基础分区，`state` 变 `MergeCompleted`，
   下一次启动**不再需要 COW**，所以能进系统。

> 诚实边界：系统首启失败时 logd 还没起来，没抓到它自己的报错（pstore 也没取到）。
> 上面第 3、4 步是**推断**，但有三条硬支撑：COW 确实建在 /data（logcat 有 create）；
> 合并后确实能起（COW 被消掉）；上游新版 UES 专门加了"recovery 里不许建 /data 快照"的回退
> —— 等于上游承认这条路走不通。

### 为什么别的机器不卡

- 官方 v0.3.1 的 annibale / byron / popsicle / myron 都是 Android 15/16，包小、super 有 COW 余量 →
  快照建在 **super** 里 → first-stage 能映射 → 系统起来后自己合并。**根本不产生"data-backed COW"这个形态。**
- dijun（同 O1）也是 Android 16、super 布局不同；没有它的刷机日志，**"它的 COW 够用"仍是推断**。
- yarp 不卡，是因为它的 UES 主动回退成直写（见上）。
- 结论修正：这不是"OS4.0.6 通病"这么笼统，而是
  **"包大到 super 装不下 COW（violin 只剩 291 MiB）+ recovery 里 /data 可用 + 旧 UES 不回退"** 三者叠加。
  高通 K90 的 issue #67 大概率也是同一个机制（包大 → COW 溢出 super），值得按这个口径去核。

### 为什么"官方 recovery 也是同一个 UES 却不炸"

stock recovery 默认**不挂 /data**（sideload 从 USB 读，不需要它）。
`/data` 不可写 → `CreateCowImage` 失败 → 旧 UES 也会走 `Attempt to overwrite existing partitions` 直写 → 能开机。
**Skyward 打开 /data 后一直挂着 rw（文件管理/读包/备份都要它）。装 `/sdcard` 上的包时 /data 必然已解密挂载，
恰好把旧 UES 的这条坏路径打开了。** 这一点是我们和其它 recovery 的真正差别。

---

## 3. 代码里的问题（按严重程度）

### P0 装完不做合并、也不检查可启动性，就允许重启

- `crates/skyward-install/src/run.rs:605-611`：`apply()` 返回 0 → 只写回 recovery + 返回 `Installed{slot}`，
  **从不读回 `/metadata/ota/state`**，不判断目标槽位能不能启动。
- `crates/skyward/src/ui.rs:2049-2066`：`Work::Update => (installed, …)`，`installed==true` 就
  `leave_at = now + 1500ms` → 1.5 秒后 `power(System)`；从页面刷包则停在完成页等用户自己重启。
- 两条路一样：**没有任何一步在重启前把 COW 合掉，也没有任何一步判断目标槽位能不能启动**。
- 结果：装完必重启（自动或手动），重启必卡，卡完必须再进一次 recovery 才有 `begin()` 的兜底合并。
- 注释 `ui.rs:2035-2039` 只说"槽位由 update_engine 设，别再设一次"，**没说清"所以我们必须在这里先合并"**，
  读代码的人会以为重启是安全的。

### P1 `slot.other()` 是猜的，`reflash` 的注释与调用方不一致

- `run.rs:608` / `run.rs:610` 用 `slot.other()` 当"下一个启动的槽位"；
- 同一文件 `run_script()` 里却老老实实问 `skyward_vab::boot::next_slot()`（`run.rs:908`）；
- `reflash()` 的文档（`run.rs:630-632`）写的是"into that of `next`, the slot that starts next"，调用方传的却是 `other`。
- 现在 UES 确实会切槽（01:31:26 有 `SetActiveBootSlot`），所以**暂时没炸**；但一旦某次 UES 没切/同槽更新，
  Skyward 会被写进错的 recovery，页面"已刷入槽位X"也是错的。

### P1 `Overwriting` 的语义是反的，失败页会吓人

- `crates/skyward-install/src/ota.rs:43-46`：注释写"Super has no room for snapshots, so it writes over the
  partitions of **the slot the phone runs from**"。
- 实际那句 `Will overwrite existing partitions. Slot B may be unbootable until update finishes!` 说的是
  **目标槽位**（yarp 日志里跑在 A、写 B，原文就是 Slot B）。触发条件也不只是"super 没空间"，
  而是"快照准备失败"（含 /data 不可用、策略拒绝）。
- 这个错误语义一路传到 UI：`skyward-ui/src/app/install.rs:83-85,111-112,266` 把 `overwriting` 当成
  "在写当前系统"，于是 `pages.rs:1687-1692` 在失败时告诉用户
  **"当前系统已被部分覆盖，重启后将无法进入系统"** —— 这句话是假的：当前槽位完好，坏的是目标槽位。

### P1 合并失败 / 合并卡住都没有出路

- `skyward-vab/src/lib.rs:386`：`MergeFailed → Standing::Failed`；
- `ui.rs:1594-1623`：`begin()` 只处理 `Merging`（且要求 `snapshots_mapped`）和 `Waiting`；
  `Failed` 什么都不做；
- `pages.rs:2744`：只有 `Standing::Waiting` 画滑动合并，`Failed` 只有一行红字，没有按钮；
- `ui.rs:1596` 如果 `Merging` 但 `snapshots_mapped()==false`，merger 压根不启动，页面永远停在 "正在合并 0%"，
  也没有任何操作。
- 即：**一次合并失败 = 只能重刷**。而 `snapshots::check()` 本来就能从 `MergeFailed` 续合（`snapshots.rs:657-658`），
  只是没人调用它。

### P2 `Effect::Merge` 仍然能一句话把整个 recovery 带崩

- `ui.rs:2772-2783`：`self.merger = Some(merge::Merger::start(slot, true)?);`
- 幂等分支已经加了（`merger.is_some()` 直接返回），但其它 `Merger::start` 失败（binder 没起、
  boot control 不在、`/metadata` 不能写）仍然 `?` 冒泡 → 整个页面线程结束。同类问题在
  `begin()` 的 `Merger::start(slot, true)?`（`ui.rs:1615`）也存在，而 `begin()` 失败等于 recovery 起不来。
- 期望：失败只在页面报一句 + kmsg，recovery 继续可用。

### P2 不可逆的合并没有任何确认

- `ui.rs:1600-1620`：每次启动只要 `Waiting` 就自动合并，日志里写"用户装包时已同意"。
- 但"同意装包"和"同意不可逆合并"是两件事：合并会消掉回滚到更新前系统的可能。
  至少应在安装页/完成页把话说清楚（现在是全靠 kmsg）。

### P2 文案/注释把"重启后系统自己会合并"当事实

- `skyward-ui/src/fmt.rs:418-441`：`Standing::Installed` → "重启后将从槽位 X 启动新系统，启动后自动合并"；
- `skyward-vab/src/lib.rs:348-352`：`Installed` 文档 "it merges once the system there has started"；
- `ui.rs:1604`：`so a plain reboot after installing the package works`（事实是"再进一次 recovery 才 work"）。
- 这三处都是同一个未经验证的假设；在 violin + OS4.0.6 + 大包 上它是**假的**。
  建议改成中性表述（"若新系统能启动，由系统合并；否则在本页合并"），或在装完主动合并后它们才成立。

### P3 其它

- `skyward/src/merge.rs:1-4,18-21`：文档写"merge into **the slot the phone runs from**"，
  但 `run(slot, …)` 只按传入槽位干活，代码并不校验；`snapshots::initiate` 的报错文案也是这个口径
  （`snapshots.rs:559-568`）。如果按第 4 节在**另一个槽位**上合并，这些注释必须先改，否则会误导下一个人。
- `snapshots.rs:654-663`：`state==MergeCompleted` 时又 `set_state(MergeCompleted)` 写一遍，
  多一次 bootloader 通知（无害，但没必要）。
- `ui.rs:1040-1057`：`ro.product.mod_device` 缺失时回退 `ro.product.device` 并注释"服务器只需要设备名"——
  这是**假设**。如果哪天给非国行机刷，可能查到别的区域的包。至少注释要标成"国行可接受"。
- `ui.rs:1057`：`unlocked = boot_param("androidboot.flash.locked")==Some("0")`，参数缺失按"锁定"算；
  和 0935 修 `avbctl` 的那个坑是同一类（缺失 ≠ 1）。影响的是上报给更新服务器的字段。

---

## 4. 修复建议（按推荐顺序）

### 方案 A（推荐）：装完、重启前就地合并

改动集中，不动 UES：

1. `skyward-vab/src/partitions.rs` 新增"只映射不挂载"：
   ```rust
   /// 把该槽位的快照逐个 map 成内核快照设备（不挂载），供另一个槽位的合并使用。
   pub fn map_snapshots(slot: Slot) -> Result<Vec<String>>;
   ```
   内部用现成的 `snapshots::live()` + `snapshots::map_snapshot()`，不要 `mount_ro_over`。
2. `skyward/src/ui.rs` 安装结束分支（`Told::Finished(Outcome::Installed)`）：
   - 读 `skyward_vab::update()`：`state==Unverified && source==当前槽` ⇒ 目标 = `source.other()`；
   - `map_snapshots(target)`；`self.merge_slot = Some(target)`；`Merger::start(target, true)`；
   - **不要**再设 `leave_at`；把页面切成"正在合并"，合并完成后再 `leave_at`（或直接 `power(System)`）；
   - `merge_told()` 用 `merge_slot.unwrap_or(info.slot)` 读 `standing()`。
3. 文案：安装页/完成页加一句"合并会把更新写死，之后不能回到更新前的系统"。
4. 失败处理：合并失败 → 停在页面 + 提供"重启（可能无法进系统）/重试合并"，别把人锁死。

代价：多 30-90 秒（这次 COW 约 9.2 GiB，UFS 写回）。收益：一次刷机一次重启就能进系统，和 yarp 体验一致，
且不依赖第三方二进制。

### 方案 B（最省事）：装完不自动重启

只改 `ui.rs:2065`：`Outcome::Installed` 且目标槽位 `Unverified` 时不设 `leave_at`，
页面显示"需先合并才能重启"，给一个合并按钮。
缺点：还要用户点一下；但比"卡一次 Mi + 手动进 recovery"好得多。

### 方案 C（治本，先做实验）：让旧 UES 走直写

在 `apply()` 之前，把 `/data/gsi` 用一个**只读 tmpfs** 盖住：
```rust
skyward_block::mount::mount("tmpfs", "/data/gsi", "tmpfs", "ro,size=1m")?;
```
UES 建 COW 镜像必然失败（EROFS），就会走 `Attempt to overwrite existing partitions` 直写实体分区，
`state` 保持空 → 新槽位开机即可用，**不需要合并**，也最接近 stock/yarp 的行为。
- 前提：`update_system()` 已经要求 `snapshot_state()==None`，所以不会盖住正在使用的更新；
- 风险：依赖 UES 内部回退路径（yarp 日志证明 ENOSPC 会回退，EROFS 需要真机验证一次）；
- 验证方法：刷完不重启，`ls /metadata/ota` 应为空、`snapshots/` 无内容，重启应直接进系统。

### 方案 D（不推荐）：换 yarp 的 `update_engine_sideload`

能立刻复刻 yarp 的行为，但那是第三方系统组件，且只是"用回退掩盖 COW 不足"。

---

## 5. 注释勘误表

| 位置 | 现在写的 | 实际 |
|---|---|---|
| `ota.rs:44-45` | 写到 "the slot the phone **runs from**" | 写的是**目标槽位**（Slot B 那条日志就是在 A 上跑着写的 B） |
| `ota.rs:44` | 原因只有 "Super has no room" | 快照准备失败都算（/data 不可建、策略拒绝、空间不足） |
| `app/install.rs:83-85,111-112` | `overwriting` = 在写当前系统 | 是目标槽位被直写，当前系统不受影响 |
| `pages.rs:1689` | "当前系统已被部分覆盖，重启后将无法进入系统" | 当前系统完好；是**目标槽位**半写、起不来 |
| `ui.rs:1604` | "so a plain reboot after installing the package works" | 事实是要再进一次 recovery 才 work |
| `ui.rs:2035-2039` | 只解释"为什么不再设 active slot" | 少了结论：所以必须在重启前合并 |
| `fmt.rs:421-426` / `vab/lib.rs:348-352` | "重启后…启动后自动合并" | 该槽位在 COW 未合并前**起不来**，这句只在能启动时成立 |
| `merge.rs:1-4` / `snapshots.rs:561` | "merge into the slot the phone runs from" | 代码按传入槽位合并，不要求是当前槽位（方案 A 就会跨槽） |
| `run.rs:630-632` | reflash 写进 "`next`, the slot that starts next" | 调用方传的是 `slot.other()`，不是问出来的 next |

---

## 6. 还需要的一次真机取证

想 100% 坐实"first-stage 打不开 /data 上的 COW"，只差一份**系统首启失败时的 console**：

1. 内核如果带 pstore/ramoops：失败后进 Skyward，`cat /sys/fs/pstore/console-ramoops*`；
2. 或临时把 `androidboot.console=ttyMSM0` 之类打开，用串口/UART 抓一次首启；
3. 或在失败后进 Skyward 里手动模拟 first-stage：不挂 /data，只按 `lp_metadata` 建 `*-cow-img` 的 dm-linear，
   再 `dmsetup create system_a --table "0 N snapshot …"`，看是否报 `Can't open COW device`。

这三条任一条拿到，就能把第 2 节第 3 步从"推断"升级为"实证"。
