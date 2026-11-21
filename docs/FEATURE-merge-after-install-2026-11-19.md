# 装完即合并（方案 A）：实现与验证

日期：2026-11-19
前置分析：`docs/REVIEW-boot-after-ota-2026-11-19.md`

## 1. 一句话

刷完官方包后，Skyward 在**同一次安装里**就把写进另一个槽位的 COW 合并掉，
完成页说"安装成功"时新槽位已经能自己启动；重启一次直接进新系统，
不再需要"卡 Mi → 再进 recovery → 手动合并 → 再重启"。

## 2. 为什么必须在这里合并

- update_engine 把官方包写成**内核快照**：`/metadata/ota/snapshots/*` 记状态，
  COW 数据落在 **/data/gsi/ota/*-cow-img.img**（super 只剩 291 MiB，这个包要 9.2 GiB）。
- 新槽位第一次启动时，first-stage init 必须**先打开这些 COW 镜像**才能建 dm 表；
  那一刻 /data 还没解密 → 建不起来 → 卡在 Mi 图标。
- 合并把 COW 写回基础分区后，启动不再需要 COW，所以能起。
- stock recovery 不挂 /data，旧 UES 会走"直写"回退，因此不卡；
  Skyward 为了读包/文件管理一直挂着 /data，恰好把这条坏路径打开。
- 以前唯一的补救是再进一次 Skyward（那时 /data 已解密）在更新页合并。
  现在把这一步搬到"安装完成前"，用户只需要重启一次。

## 3. 改动

### 3.1 `skyward-vab`：把"合并"做成可复用的两步

| 位置 | 内容 |
|---|---|
| `src/merge.rs`（新） | `follow(slot, start, each)`：开 binder → BootControl → `/metadata` 挂 rw → `start_merge` → 每 2 s `check_merge` 直到结束 → `/metadata` 挂回 ro。原来在 `skyward/src/merge.rs` 里的逻辑整体下移，**槽位参数不再假设是当前槽位**。 |
| `src/partitions.rs` | `map_snapshots(slot)`：把该槽位的每个快照映射成内核快照设备，**不挂载**；已经是内核快照的（UES 留下的）原样保留，否则先 `unmap_snapshot` 再 `map_snapshot`。返回映射了哪些分区。 |
| `src/snapshots.rs` | `initiate()` 的形参由 `current` 改名 `target`，报错文案改为"不是更新写入的槽位"；允许从 `MergeFailed` 重新发起（重试合并）。 |
| `src/lib.rs` | 导出 `map_snapshots`、`merge` 模块；新增 `unmerged_target(current)`（更新还等着合并的槽位：当前槽位或另一个）与 `Standing::starts()`（这个槽位能不能启动）。 |

### 3.2 `skyward-install`：装完就合并

`update_system()` 在 update_engine 返回 0、写回 Skyward 之后，调用新增的
`merge_written(ctx, current)`：

1. `unmerged_target(current)` 说没东西可合并（直写、已合并、无更新）→ 直接返回；
2. `map_snapshots(target)` 映射目标槽位的快照，一个都没有 → 返回；
3. `merge::follow(target, true, …)`，每 2 s 把 `Merge::Going(share)` 变成
   `Event::Progress(Stage::Merging { thousandths })` 发给安装页；
4. 结束（`Done` / `NeedsReboot`）→ kmsg + 1000‰；
   失败只记 kmsg，**不改安装结果**——安装确实成功了，滚动条之外的状态由更新页说。

`Stage::Merging { thousandths }` 是新增的进度阶段。

### 3.3 `skyward`：重启前的判断与更新页的重试

- `installer_told()`：`Finished(Outcome::Installed)` 时先读一次 `standing(slot)`
  塞给页面，再决定 `Work::Update` 要不要自动重启——
  **只有 `Standing::starts()` 为真才自动重启**（`None`/`Merged`/`Compressed`）。
  未合并/合并失败时留在完成页，不再自动重启进一个起不来的槽位。
- `Effect::Merge`（更新页滑动合并）：先用 `unmerged_target` 找到目标槽位，
  另一个槽位就先 `map_snapshots`，再 `Merger::start(target, true)`；
  不再死认"当前槽位"。
- `begin()`：`Standing::Merging` 且合并在**另一个槽位**（安装后合并被断电打断）
  时，重新 `map_snapshots(target)` 并接着跟（`start=false`），不再判 `snapshots_mapped(current)`；
  `Standing::Waiting` 分支也先 `map_snapshots(slot)` 再起合并，补上"快照还没映射
  就点合并"的死路。

### 3.4 界面

- **安装进度页**：新增"正在合并更新 N%"，替代"第 x 步，共 3 步"；
  底部提示改为"正在把更新合并到新系统的分区，约需几分钟。合并完成前请勿重启或断电"。
  合并期间返回键仍然拒绝离开（`Install::Updating` 既有规则）。
- **完成页**：`state.update` 说还不能启动时，卡片变橙色"安装成功，待合并更新"，
  **不再给"重启到系统"**，改成"合并更新"直达「高级 > 系统更新」；
  合并失败另有提示。
- **更新页**：`Standing::Installed` / `Failed` 也画出"滑动以合并"（以前只有 `Waiting`）；
  `fmt::update()` 里"重启后自动合并""重启后将继续合并（ReadStatus）"这类会误导的文案
  改成"尚未合并；合并后重启即进入新系统"。

## 4. 日志标记（真机核对）

```
install: install: …                                    # 原有
install: 10 snapshots of slot B are mapped, merging them so the slot starts
install: the update is merged into slot B; it starts on its own
```
- 失败时：`install: the update in slot B did not merge: …`
- 页面事件：`install Progress(Merging { thousandths: … })`（`eprintln!("install {ev:?}")`）。
- 更新页/开机续合：`the merge into slot B was cut off; it goes on`。

## 5. 真机验证清单

1. 刷本镜像进 recovery，**刷官方完整包**：
   - 进度页在"第 3 步"之后出现"正在合并更新 N%"，持续几十秒到几分钟；
   - 完成页显示绿色"安装成功"，下面只给"重启到系统"；
   - `adb shell` 不可用时可拔线，但**合并期间不要断电**。
2. 重启 → 应直接进 HyperOS 4.0.6，**不再卡 Mi**。
3. 失败预案：若完成页是橙色"安装成功，待合并更新"，点"合并更新"→ 滑动合并；
   合并成功后返回完成页即变绿，可重启。
4. 断电演练（可选、可逆）：合并中途长按电源断电，再进 Skyward，
   日志应出现 `the merge into slot B was cut off; it goes on`，更新页显示合并进度，
   跟完后即可重启进系统。
5. 回归：双击亮屏、锁屏右滑、WLAN 自动连接、MTP（解密后自动 `mtp,adb`）、90 Hz。

## 6. 影响面与风险

- **时间**：多一次合并（COW 有多大就多久，实测同类操作 30 s～2 min），
  一次刷机总时长增加，但省掉一次"卡 Mi + 再进 recovery + 再重启"。
- **不可逆**：合并写入目标槽位的基础分区，更新前的那个槽位内容不再保留；
  这与"装完重启由系统自己合并"的语义一致，日志有明确告知。
- **不动当前系统**：合并只写目标槽位（安装时已整体覆盖），当前槽位分区不参与，
  合并失败也不影响"回到旧系统"。
- **snapuserd（压缩快照）**：`Standing::Compressed` 不在这里合并（recovery 不跑 snapuserd），
  仍按原样交给系统；完成页保留原有提示，也仍然允许重启。
- **回退**：`out/skyward-violin-OS4.0.5-20261119-0935.img`（本轮之前）。

## 7. 构建

| 步骤 | 结果 |
|---|---|
| `cargo build --release --target aarch64-linux-android -p skyward` | RC=0 |
| `cargo build --release -p skyward-pack` | RC=0（仅 `skyward-bootimg` 既有 4 个 cfg 桩 warning） |
| `skyward-pack recovery --stock .references/stock405-recovery.img …` | 见 `docs/BUILD-merge-after-install-2026-11-19.md` |
| `cargo clippy … -D warnings` | 本轮的 crate 无告警；`skyward-ui` 有 6 条**改动前就存在**的 lint（`input.rs` 折叠 if、`state.rs` chunks_exact、`app/mod.rs` div_ceil、`settings.rs` Default/大枚举、`widgets.rs` 参数过多），未在本轮处理。 |
