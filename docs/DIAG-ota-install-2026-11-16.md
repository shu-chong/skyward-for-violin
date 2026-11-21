# 官方 OTA 刷入诊断：系统成功，仅 Skyward 写回 recovery 失败

日期：2026-11-16 23:00
现场日志：
- `.references/logs/live-install-20261116-225252.txt`（实时跟随，用户中途取消）
- `.references/logs/install2-20261116-225619.txt`（78,784 行，一次性缓冲）
- 设备内 `/skyward/run/update_engine.log`（166 KB）、`/skyward/run/recovery.log`

本轮**只读诊断**，设备未写入、未刷机。

## 1. 结果：系统刷成功了，报错只影响 Skyward 自身

```
14:52:17 skyward: install: /sdcard/Documents/violin-ota_full-OS4.0.6.0.XOTCNXM-...zip, 8770320010 bytes
14:52:17 skyward: install: an update to 17OS4.0.261112.190429467.XRPDCN.S
14:58:15 update_engine: Update successfully applied, waiting to reboot.
14:58:17 skyward: install: this recovery written back to [B]:
                        Err(write /dev/block/by-name/recovery_b: Operation not permitted (os error 1))
14:58:17 skyward: install: Installed { slot: B, wipe: false }
```

- **新系统完整写入 B 槽**，update_engine 报 `kSuccess`，unmap 全部动态分区，写完成标记 `slot 0`
  （`update_engine` 内部把 a 记为 0；Skyward 侧 `slot.other()` 为 B，两者指向同一个目标槽）。
- 失败的是 `reflash()`：把当前 Skyward 写回 **B 槽的 `recovery_b`**，被内核以 **EPERM (os error 1)** 拒绝。
- 整体 outcome 仍是 `Installed`，所以屏幕显示「安装成功」+ 底部一行「未能将 Skyward 重新刷入 recovery 分区」。
- `recovery_b` 头部仍是 `ANDROID!`，分区未被写坏；**当前槽 a 的 recovery（Skyward）也完好，设备仍可进 recovery**。

### 排除了什么

- 不是签名校验失败、不是 payload 损坏、不是空间不足、不是槽位判断错误。
- 不是块设备只读：`/sys/class/block/sdc26|sdc55/ro` 都是 `0`；`holders` 为空；
  非破坏性 `dd ... count=0` 以写方式打开 `recovery_a/b` 都成功（没写数据）。
- 不是 SELinux：`avc denied ... permissive=1` 是审计不是拒绝。
- 不是 checkpointing：`unable to start transaction in checkpointing` 每步都出现，
  update_engine 明确 `ActionProcessor: finished ... with code ErrorCode::kSuccess`，是警告。

### "1 秒显示 100%" 是显示 bug，不是真写完了

`apply()` 用 `mount.served().saturating_sub(payload_offset).min(total)` 当进度：FUSE 把整个 8.77 GB
包几乎瞬间读完，served 立刻达到 total，UI 秒到 100%。真实进度在 `/skyward/run/update_engine.log`：

```
14:56:45 Completed 4109/5180 operations (79%), 7191658496/8770311080 bytes (82%), overall 80%
14:57:15 Completed 4460/5180 operations (86%), 7702806528/8770311080 bytes (87%), overall 86%
14:58:15 ActionProcessor finished with code ErrorCode::kSuccess
```

update_engine 自己的 `overall progress` 才是真的。**建议下一轮改用它的状态行驱动进度条。**

### EPERM 的最可能原因

`reflash()` 在 update_engine 结束**之后**立刻写 `recovery_b`。玄戒 O1 的
`ab_slot_bootable_ctrl.ko` / `kmod_protect` 在 OTA 完成瞬间仍持有 recovery 分区的保护，
或 `recovery_b` 此刻仍被内核映射；dmesg 缓冲里没有留下具体拒绝行（被后续 wlan 审计刷掉）。
这是平台行为，不是代码写错分区：**目标槽 B 是对的**。

## 2. 已改：刷机时侧栏不再能点走页面

你指出的「刷机时可以点左边换页」是真实绕过点：

- 键盘和返回键早就被 `nav.rs::back()` / `act.rs` 拦住（Installing 时提示等待），
  但平板侧栏走的是另一条路：`ui.rs` 输入分发里 `tablet::Input::Select(screen)`
  直接调 `app.select_section()`，**没有忙碌检查**。
- 已加 `App::navigation_open()`（`screen() != Screen::Installing`），
  侧栏选中在 `Installing` 时直接忽略。等这一轮构建后生效。

## 3. AVB commit 评估（`15bb3f70`，与本次刷机无关）

`TWRP-Test/android_bootable_recovery@15bb3f70` 只加一个**编译期** flag
`TW_AVB_VBMETA_FLAGS_ALL_DISABLED`（Android.mk 的 `LOCAL_CFLAGS`），
让 `Disable_AVB2()` 从只设 `HASHTREE_DISABLED` 改为
`HASHTREE_DISABLED | VERIFICATION_DISABLED`，再写 `vbmeta_a/b`。**它不是「高级」菜单项**。

本机只读状态：

```
avbctl get-verity       → disabled on slot _b
avbctl get-verification → enabled on slot _b
ro.boot.vbmeta.device_state → unlocked
ro.boot.verifiedbootstate   → orange
init: [libfs_avb] status: HashtreeDisabled
```

Skyward **已有完整底层能力**：`/system/bin/avbctl` 支持
`get/disable/enable-verity|verification`，`set()` 只改 vbmeta 头 4 字节 flags、带 locked 检查；
Advanced 页目前只有 6 个工具项，缺一个 UI 入口。

三个方案（未动，等拍板）：**A** 不改代码，用现成 `adb shell avbctl disable-verification`；
**B** Advanced 加「关闭 AVB 验证」滑动确认 + 警示 + 恢复按钮（可选双槽）；
**C** 只加只读状态显示。写 vbmeta 属启动关键分区，且 HyperOS 可能要求清 data，故不擅自做。

## 4. 建议的下一轮修复

1. **reflash 失败改为非致命**：已经是非致命（outcome 仍 Installed），但可以把提示改成
   「Skyward 未能写回 recovery，请用 fastboot 单独刷 recovery_b」，避免用户误以为刷机失败。
2. **进度条改真进度**：解析 update_engine 的 `overall progress N%` 而不是 FUSE served。
3. **侧栏拦截**（已改，待构建）。
4. AVB 选 A/B/C 后实施。

回退镜像：`out/skyward-violin-OS4.0.5-20261116-0950.img`（`8582f695…aec5f`）。
