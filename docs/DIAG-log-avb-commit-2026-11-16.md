# 真机 log 复查 + TWRP「去除 AVB 验证」commit 评估

日期：2026-11-16 09:58
抓取：`.references/logs/dmesg-20261116-0958.txt`（6,180 行）、`logcat-20261116-0958.txt`（26,192 行）
设备：recovery，`sys.usb.config/state = mtp,adb`，`persist.sys.usb.config = adb`

本轮**只读诊断**：没有改代码、没有打包、没有刷机、没有写任何设备分区。

## 1. log 结论：没有影响功能的错误

| 指标 | 结果 |
|---|---|
| `underflow` / `signal 6` / `recovery failed` | 0 / 0 / 0 |
| `xr_usb_composite_disconnect` | **1 次，01:54:46.931（解密时刻，预期内）**，开机 0 次 |
| `sys.usb.state` | `mtp,adb`（当前），开机是 `adb` |
| `init.svc.adbd` | `running` |

### C 方案（解密后自动 MTP）全链路已验证

```
01:54:15.379  init: sys.usb.config=adb            ← 开机只跑 adb
01:54:15.287  init: ... from skyward-mtp.rc:4      ← 只建 functionfs 节点，不重绑
01:54:34.315  skyward: /dev/block/dm-10 holds f2fs at /data
01:54:34.315  skyward: the users whose own files are still closed: [0]
01:54:46.926  skyward: the users whose own files are still closed: []   ← 解密完成
01:54:46.931  xring_usb: xr_usb_composite_disconnect                    ← 预期的一次重绑
01:54:46.967  init: sys.usb.config=mtp,adb (skyward-mtp.rc:9/12)
01:54:47.007  skyward: file transfer came up once data was decrypted
01:54:47.345  skyward: mtp: the computer configured the function
01:54:47.405  skyward: mtp: the computer opened a session               ← 电脑已连上 MTP
```

顺带确认 ⑦ WLAN 自动连接也生效：`the saved WLAN is being joined again` →
`wlan0 has 192.168.31.173` → `the network on wlan0 reaches the internet`；
`the settings are kept` 说明 /metadata 持久化正常。

### 日志里那些「error / failed」都是原厂遗留，不是回归

- `avc: denied … permissive=1`：SELinux 是 permissive，**审计记录不是拒绝**，解密时读取
  `/data/system/locksettings.db` / keystore 等的那一批，历版都有，解密最终成功。
- `i2c i2c-6: Failed to register i2c client SC6601 at 0x61 (-16)`、`dse: Failed to get array
  length: name cmes ret -22`、`vsensor/ntc_tm: failed to register sensor id=5`：内核驱动
  重复注册/探测失败，stock 内核自带，与 recovery 无关。
- `recovery-refresh` / `recovery-persist` 找不到：原厂 ramdisk 的 rc 引用了这两个不存在的程序
  （`init: ... failed: Cannot find ...`），所有版本都有，不影响功能。
- 电量计 `mca_strategy_fg` 的 `-E` 行只是日志级别，内容全是 `master_error:0 / slave_error:0`；
  WLAN `Link Quality` 里的 `fail:` 是重传统计、`PER(0)`。
- 时钟未设置（`persist's time/ats_2` 不存在）是已知的开机提示。

## 2. 那个 TWRP commit 到底做了什么

`TWRP-Test/android_bootable_recovery@15bb3f70`（`custom: Disable AVB2.0: Add
TW_AVB_VBMETA_FLAGS_ALL_DISABLED option`）只改 2 个文件、共 9 行：

- `Android.mk`：加一个**编译期** `LOCAL_CFLAGS += -DTW_AVB_VBMETA_FLAGS_ALL_DISABLED`。
- `partitionmanager.cpp`：`Disable_AVB2()` 原本只设
  `AVB_VBMETA_IMAGE_FLAGS_HASHTREE_DISABLED`；带该 flag 后改成
  `HASHTREE_DISABLED | VERIFICATION_DISABLED`，然后 `Do_Disable_AVB2("vbmeta_a", flags)`
  / `vbmeta_b` 写两个槽的 vbmeta 头 flags。

**它不是新增的「高级」菜单项**，只是让 TWRP 已有的 Disable AVB 动作多关一个 bit。
对应到我们机器：**verity（bit0）已经关了，缺的是 verification（bit1）**。

### 本机 AVB 现状（只读检查）

```
avbctl get-verity           → verity is disabled on slot with suffix _b.
avbctl get-verification     → verification is enabled on slot with suffix _b.
ro.boot.vbmeta.device_state → unlocked
ro.boot.verifiedbootstate   → orange
init: [libfs_avb] … status: HashtreeDisabled
/dev/block/by-name/         → vbmeta_a/b, vbmeta_system_a/b, vbmeta_vendor_a/b 都在
```

### Skyward 其实已经有完整底层能力

`crates/skyward-install/src/commands/avbctl.rs` 已实现 `/system/bin/avbctl`，
设备上可直接调用（已验证 usage）：

```
avbctl [--force] get-verity | disable-verity | enable-verity
                 get-verification | disable-verification | enable-verification
```

`is_on()` / `set()` 读写 vbmeta 头 256 字节、只改 `flags` 那 4 字节，其余原样保留；
带 bootloader locked 检查（locked 时拒绝，需 `--force`）。
`crates/skyward-ui/src/pages.rs::advanced()` 目前只有文件/终端/修复 SELinux/文件系统/日志/更新
六个入口，**没有 AVB 开关**。

也就是说：**能加，而且不用重写写盘逻辑，只差一个 UI 入口。**
甚至现在就能不改代码直接用：`adb shell avbctl disable-verification`（可逆：
`adb shell avbctl enable-verification`）。

## 3. 方案与风险（等确认，本轮未动）

| 方案 | 内容 | 代价 |
|---|---|---|
| **A 不改代码** | 用现成的 `adb shell avbctl disable-verification` / `enable-verification` | 每次要连电脑敲命令；只写当前槽 `_b` |
| **B 加高级页按钮**（对应你说的「高级 去除 AVB 验证」） | 在 Advanced 加一行「关闭 AVB 验证」+ **滑动二次确认** + 明确警示，同时提供「恢复验证」；复用 `avbctl::set()`，可选两个槽都写 | 要写 `vbmeta` 分区头，属启动关键分区 |
| **C 只加只读状态** | 高级页显示当前 verity / verification 开或关，不写盘 | 零风险，但不能改 |

### 必须说清的风险

1. **会写 vbmeta 分区**（不是 system 分区，但属于启动验证链）。你最初要求「所有操作以不损坏设备为前提」，
   所以这一步我不擅自做。
2. 本机 bootloader 是 `unlocked`，理论上关 verification 后仍能启动；但 HyperOS / 玄戒平台
   可能有额外校验，改 vbmeta flags 后首次启动**有可能要求清除 data 或无法进系统**。
3. `avbctl` 默认只写**当前槽** `vbmeta_b`；切到 a 槽后验证会回来。要对齐 TWRP 的
   `vbmeta_a + vbmeta_b` 双槽写入，需要在 UI 里额外处理（B 方案可做）。
4. 可逆：只要还能进 recovery，用 `enable-verification` 或 B 方案的「恢复验证」能改回来。
5. 若 bootloader 是 locked，`avbctl` 会拒绝（除非 `--force`，**不建议**）。

## 4. 建议

先告诉我选 **A / B / C**。若选 B，我建议的实现是：
Advanced 页新增一行开关显示当前状态 → 点进去是一个说明页 + 滑动确认 →
`Effect` 调 `avbctl::set()`（默认只写当前槽，另给「两个槽都写」选项）→ 结果写 kmsg 并在页面提示，
全程不碰 `boot / system / 分区表`，且提供一键恢复。

回退镜像不变：`out/skyward-violin-OS4.0.5-20261116-0950.img`（`8582f695…aec5f`）。
