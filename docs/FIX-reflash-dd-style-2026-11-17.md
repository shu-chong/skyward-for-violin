# B 方案：刷官方包后按 yarp 方式写回 recovery（dd 式 + 重试）

日期：2026-11-17
镜像：`out/skyward-violin-OS4.0.5-20261117-0022.img`
SHA-256：`fbc3b6659eda6cc49a40e4371626a9cd912d7b32c20f607bb8e6cc277a871d31`
本轮**未刷机、未写设备分区**。

---

## yarp 是怎么做的（log 实证）

`/tmp/recovery.log`（设备只读抓取）：

```
1689  operation_start: 'Disable AVB2.0'
1699  operation_start: 'Flashing'                     ← 刷官方 ROM
3211  Update successfully applied, waiting to reboot.
3219-3308  Unmounting /vendor/mitee/ta, /odm/...       ← 写回前先卸载
3343  请重启 TWRP 切换到新槽位后，再刷入其它的 Zip。
3344  dd bs=1048576 if=recovery_b of=recovery_a        ← 从保留槽复制到被覆盖槽
3345  dd ... RC=0
3350  Install took 271 second(s)
```

当前从 B 槽启动。OTA 把新 recovery 写进**新系统槽** `recovery_a`，而当前槽 `recovery_b` 仍是 TWRP；
yarp 用裸 `dd` 把 `recovery_b` 复制到 `recovery_a`，配合 `switch_slot_on_reboot=true` 重启切槽。

## Skyward 为什么失败

Skyward 的 `reflash()` 在 OTA 一结束就从**内存镜像**（安装前读的）写回目标槽，报
`write recovery_b: Operation not permitted`。差异：
- yarp **从另一个仍持有 recovery 的分区复制到目标槽**（块设备→块设备），且写前先卸载、等待；
- Skyward 从内存写，且写得早（OTA 刚结束、内核可能还持有分区）。

两者路径都是 `/dev/block/by-name/recovery_*`；真正差别在**数据来源 + 时机**。

---

## 改动（B 方案）

### keep.rs
- `Kept::source()`：安装后找出仍持有本 recovery（digest==own 且是 Skyward）的槽——即没被 OTA 碰过的那个。
- `Kept::write_from_device(slots, progress)`：从该源槽分区读镜像，写到目标 `slots`（等价 dd）；
  若没有源槽或源槽本身就是目标，返回 `Ok(false)` 让调用方回退到内存写。

### run.rs `reflash()`
- 改为先 `write_from_device()`（dd 式）；`Ok(false)` 回退到原来的 `write()`（内存镜像）。
- 加**重试**：`REFLASH_TRIES=4`、`REFLASH_WAIT=750ms`。OTA 结束瞬间写可能 EPERM，等一下再写常能成功；
  每次重试写 kmsg `writing recovery back did not work yet (...), retry N`。
- 进度反馈、结果提示逻辑不变。

### 不变
- 无新增卸载分区动作（`unmount_partitions` 会卸载 /metadata，影响后续读更新状态，故不用；改用重试应对时序）。
- 切槽仍由 update_engine 负责（上轮已撤销我们的 set_active）。
- AVB 禁用按钮、合并提示、包名折行、真实进度、侧栏放行均保留。

---

## 验证

- Android release RC=0（1 m 04 s，无新增 warning）；宿主 pack RC=0；打包 RC=0。
- 镜像内二进制 SHA 与本地一致（`e5bd92d2…111d638`）。
- 新重试标记命中；`禁用 AVB 验证`、`更新尚未合并`、`progress now follows the bytes` 均在；切槽标记仍 0。
- `mtp.rc` 存在、`system-init` 缺席、配置不变、`repack` RC=0。

## 刷入后验证清单

1. 用本镜像重刷 recovery（`fastboot flash recovery_<当前槽> <本镜像>`）。
2. 刷一次官方完整包，完成后看底部**是否还提示"未能将 Skyward 重新刷入 recovery 分区"**：
   - 若不提示，且日志有 `this recovery written back to [B]: Ok(())` → 修复成功。
   - 若仍提示，看有没有 `writing recovery back did not work yet ... retry N`：
     有则说明确实遇到瞬时 EPERM、重试也失败，需进一步查卸载；没有则说明源槽判定/回退有问题。
3. 确认两个槽的 recovery 恢复为 Skyward（dd 后）。

回退镜像：`…20261117-0015.img`（AVB 按钮版）、`…20261117-0002.img`（撤销切槽+合并提示版）。
