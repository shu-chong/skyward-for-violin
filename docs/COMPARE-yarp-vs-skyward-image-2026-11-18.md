# YARP vs Skyward 镜像对比：为什么 yarp 刷完不用合并、Skyward 要合并

日期：2026-11-18
对比对象：
- YARP：`C:\Users\shc\Desktop\YARP-4.0.0-16-VIOLIN-20261113.img`（100 MiB，header v4，ramdisk 46,463,961 B）
- Skyward：`out/skyward-violin-OS4.0.5-20261117-2304.img`（100 MiB，header v4，ramdisk 49,190,343 B）

方法：`skyward-pack list/cat` 直接读取两边 ramdisk（两者都是它可解析的 legacy ramdisk），
导出清单到 `.references/img-compare/`，再逐文件导出关键 rc / prop / 二进制做对比。
全程只读，未刷机、未写设备分区。

## 结论（先说答案）

**yarp 能"刷完直接重启进系统"，核心不是安装方式、不是 boot HAL、不是 verity，而是：

yarp 的 recovery 里带了完整、可运行的 `snapuserd`（用户态快照守护进程），并且在刷完 A/B ZIP 后
由 TWRP 主动把虚拟 A/B 的快照合并掉（state 清零、snapshots 目录清空）；
Skyward 的 ramdisk 里根本没有 snapuserd，其合并器只支持 kernel 设备映射快照，
所以刷完留下 `state=08 02`（Unverified）+ 10 个快照，必须按音量上进 recovery 手动/半自动合并。**

一句话：**差在"快照由谁合并"——yarp 有 userspace 快照栈（snapuserd）能就地合并，Skyward 只有 kernel 快照栈、补不了 userspace 快照。**

## 决定性证据

### 1. snapuserd：yarp 有，Skyward 完全没有

| 文件 | YARP | Skyward |
|---|---|---|
| `system/bin/snapuserd` | 1,424,752 B executable | **不存在** |
| `system/etc/init/snapuserd.rc` | 537 B（含 `snapuserd` + `snapuserd_proxy` 两个 service、`on property:init.svc.snapuserd=stopped` → `snapuserd.ready=false`） | **不存在** |

`snapuserd.rc` 原文（YARP）：
```
service snapuserd /system/bin/snapuserd
    socket snapuserd stream 0660 system system
    oneshot
    disabled
    user root
    group root system
    task_profiles OtaProfiles
    seclabel u:r:snapuserd:s0

service snapuserd_proxy /system/bin/snapuserd -socket-handoff
    socket snapuserd stream 0660 system system
    socket snapuserd_proxy seqpacket 0660 system root
    oneshot
    disabled
    ...
on property:init.svc.snapuserd=stopped
    setprop snapuserd.ready false
```

两个镜像的 `prop.default` 都设了：
```
ro.virtual_ab.enabled=true
ro.virtual_ab.compression.enabled=true
ro.virtual_ab.userspace.snapshots.enabled=true
ro.virtual_ab.batch_writes=true
ro.virtual_ab.io_uring.enabled=true
ro.virtual_ab.compression.xor.enabled=true
```
即**两边都声明使用 userspace snapshots**——这需要 snapuserd 来提供 COW 视图并执行合并。
yarp 带了这个守护进程，Skyward 没有。

### 2. YARP 的 recovery 主程序内置完整的 snapuserd/libsnapshot 合并逻辑

对 `system/bin/recovery` 做 `strings` 对比（同一台机器、同一个官方 zip）：

YARP recovery（3,320,680 B，AOSP recovery/TWRP 编译，含 libsnapshot）里能搜到一整套：
```
Failed to connect to snapuserd daemon to initiate merge
Failed to detach snapuserd
Successfully restarted snapuserd in ublk mode
SwitchSnapshotToMerge: ...
Successfully switched snapshot device to a merge target
/metadata/ota/snapuserd_mode
/metadata/ota/snapuserd-from-system
Setting legacy_snapuserd to true
Detect re-flashing, cancelling unverified update.
Cannot begin a merge if an update has not been verified
```

Skyward recovery（8,846,624 B，Rust 实现）里对应的是：
```
the update's snapshots are compressed and served by snapuserd, which this recovery does not run
```
这句错误文案是 Skyward 自己写的，直白承认：**压缩快照由 snapuserd 提供，而这个 recovery 不运行它。**
它的 `Merger`（`crates/skyward-vab/src/partitions.rs::start_merge`）走的是
`Snapshots::lock(true)` + `snapshots::initiate()`，依赖 kernel 已映射的 snapshot 设备
（`snapshots_mapped()` 检查 `is_snapshot()`），**没有 userspace/ublk 那条路**。

### 3. update_engine_sideload 也是两个不同版本

| | YARP | Skyward |
|---|---|---|
| 大小 | 3,115,392 B | 2,975,424 B |
| SHA-256 | `cde4ea4a…120a87` | `609f3c21…11918` |
| 独有字符串 | `/allow-forward-merge`、`after resuming merge`、`during MergeSecondPhaseSnapshots`、`Timed out connecting to snapuserd socket` | `slot, no action is taken.` |

YARP 的是带 forward-merge / userspace-snapuserd 逻辑的新版；Skyward 用的是稍旧的那个
（来自我们打包时用的 stock 底稿）。注意 Skyward 的 UES 里也有 `Timed out connecting to snapuserd socket`，
说明它被调用时确实会尝试连 snapuserd——但 ramdisk 里没这个程序，连不上。

### 4. 结果状态对照（真机实测，前几轮已记录）

| | yarp 刷同一 OS4.0.6.0 zip | Skyward 刷同一 zip |
|---|---|---|
| 刷后 `/metadata/ota/state` | **空** | `08 02`（Unverified） |
| `/metadata/ota/snapshots/` | **0 个** | 10 个（system/product/vendor… 的 B 槽快照） |
| 重启 | 一次进系统 | 卡 Mi；按音量上进 Skyward 自动合并后重启才进 |

`state` 空 + snapshots=0 只能解释为：**yarp 在 recovery 里就把快照合并并清理了**——
这正是 snapuserd + libsnapshot 能做的事。

### 5. 附带差异（说明 yarp 为这套机制做的配套）

- YARP 的 `init.recovery.O1_asic.rc`（7,638 B）在 `on fs` 里 `start vendor.boot-xring`，
  并 `symlink /dev/block /dev/block/bootdevice`——注释明确写这是为
  `twrpRepacker::Flash_Current_Twrp()` 硬编码的 `by-name/recovery_{a,b}` 路径服务，
  即 TWRP 刷完 A/B ZIP 自动把自身写回另一槽。Skyward 同名 rc 只有 593 B，没有这些。
- YARP 带真正的 `system/bin/resetprop`（51,144 B）+ `libresetprop.so`（168,688 B），
  用于 `violin-setup.sh` 从已装 ROM 读 prop 覆盖到 recovery；Skyward 的 resetprop 只是 8 B 占位。
- boot control：YARP 同时带 HIDL `boot@1.0/1.1/1.2-service` 和 AIDL
  `boot-service.xring.recovery`；Skyward 只有 AIDL recovery 变体。
  此前我怀疑"boot HAL 变体导致卡 Mi"——**本轮证实此推断错了**：yarp 的 rc 里
  `vendor.boot-xring` 也指向 `/system/bin/hw/android.hardware.boot-service.xring.recovery`，
  与 Skyward 相同；HAL 不是差异点。

## 此前推断的修正（诚实记录）

| 先前说法 | 本轮结论 |
|---|---|
| "卡 Mi 是没切槽" | ❌ 错。update_engine 两次都自己 SetActiveBootSlot 了 |
| "是 verity / verification 全开" | ❌ 非主因。yarp 路径同样会重写 vbmeta，照样能开机 |
| "是 boot HAL 用错变体" | ❌ 错。两边 AIDL recovery HAL 相同（见上） |
| "安装方式不同（yarp 刷 img、Skyward 刷 payload）" | ❌ 错。用户确认 yarp 刷的是同一个官方 zip；zip 里只有 payload.bin，都走 update_engine |
| "官方机器靠系统内 update_verifier 自动合并" | 部分对：那是**系统正常启动**时的路径；yarp 的差异是它在 **recovery 里**就把快照合并了 |
| ✅ 正确方向 | Issue #67 指出 Unverified 无人推进——而 yarp 有 snapuserd 能推进，Skyward 不能 |

## 对 Skyward 的启示（下一步选项）

要让 Skyward 也"刷完不卡 Mi"，需要补齐 userspace 快照合并能力，可选：

- **A（对齐 yarp，治本）**：把 YARP 的 `snapuserd` 二进制 + `snapuserd.rc` + 所需
  SELinux 策略（`u:r:snapuserd:s0`）合进镜像，并在安装收尾/进入 recovery 时启动它、
  复用 libsnapshot 发起并推进合并。工作量大（要补 sepolicy、socket、ublk/kernel 模式切换），
  但这是真正的根因修复。
- **B（维持现状，已验证可用）**：保留现在的"卡一次 → 进 recovery 自动合并"，
  它已经真机跑通（用户已确认）。
- **C（折中，风险可控）**：刷完包仍在 A 槽（Standing::Installed(B)）时，主动对 B 槽
  `start_merge(B)`，把合并提前到安装结束，免去卡 Mi。风险：那时快照刚写完，`/metadata`
  还被占用，需重试/等待；且若快照是 userspace 类型，仍会撞到"Skyward 不运行 snapuserd"的限制。

推荐先 B（现状可用），A 作为长期目标；C 需先确认这次的快照到底是 kernel 还是 userspace 类型
（可从 `/metadata/ota/snapuserd_mode` 是否存在判断——本次 Skyward 刷后 state=08 02 且有 10 个快照，
倾向于 userspace，那 C 也无法绕开，只能 A）。

## 附：文件清单统计

- YARP 独有 390 项，Skyward 独有 568 项，两边共有 481 项。
- 数据文件在 `.references/img-compare/`：`yarp-list.txt`、`skyward-list.txt`、`yarp-prop.txt`、
  `skyward-prop.txt`、`yarp-ues.bin`、`skyward-ues.bin`、`yarp-recovery.bin`、`skyward-recovery.bin`。
