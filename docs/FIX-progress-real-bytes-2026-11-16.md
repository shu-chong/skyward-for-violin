# 修复「1 秒 100%」进度条 + AVB 三方案说明

日期：2026-11-16
工作区：`C:\Users\shc\Desktop\skyward-violin`（旧 `skyward` 目录全程只读）
镜像：`out/skyward-violin-OS4.0.5-20261116-2309.img`
SHA-256：`a889bc872da2d6a85d9d356e3b73ff65b48dc38b434f7eab2ef6e667a77d1b58`

本轮**未刷机、未写任何设备分区**。

---

## 1. 「1 秒 100%」显示 bug：已修

### 根因（一手日志证据）

`crates/skyward-install/src/run.rs` 原来用 `mount.served()` 当写入进度：

```rust
Stage::Writing { done: mount.served().saturating_sub(update.payload_offset).min(total), total }
```

`mount.served()` 是 **FUSE 从 OTA 包里读了多少字节**，不是 update_engine **写了多少**。
FUSE 在 sideload 开始瞬间就把整个 8.77 GB 包读完喂给引擎，所以 `served()` 立刻逼近
`total`，UI 几百毫秒就跳到 100%。而 `update_engine.log` 显示真实写入是慢慢推进的：

```
14:54:47  Completed 2022/5180 operations (39%), 3336273920/8770311080 bytes downloaded (38%)
14:56:12  Completed 3570/5180 operations (68%), 6314639360/8770311080 bytes downloaded (72%)
14:57:15  Completed 4460/5180 operations (86%), 7702806528/8770311080 bytes downloaded (87%)
14:58:05  Completed 5180/5180 operations (100%), 8770311080/8770311080 bytes downloaded (100%)
```

用户看到 100% 时实际只写了一部分，此时强制重启会把设备留在半刷写状态 —— 这正是你担心的黑砖路径。

### 改法

- `crates/skyward-install/src/ota.rs`：新增 `Log::Progress { done, total }`，
  从引擎日志 `… bytes downloaded (38%), …` 里解析 `3336273920/8770311080` 这对真实写入字节。
  不带 `bytes downloaded` 的 `Completed 10/400 operations` 行仍返回 `None`（旧测试保持通过）。
- `crates/skyward-install/src/run.rs`：`apply()` 循环改用引擎上报的 `done` 作为
  `Stage::Writing`，不再用 `mount.served()`；`mount` 参数改名 `_mount`。
  第一次收到真实进度时写一条 kmsg：
  `install: progress now follows the bytes update_engine wrote, of <total>`，
  方便真机 log 里确认这一版生效。
- 进度显示 `update_stage()` / `share(done,total)` 一行没动 —— 只换 done 的来源即可。
- 保留 2301 版的侧栏拦截（`navigation_open()`），源码仍在。

### 效果与边界

- 进度条现在跟随引擎真实写入字节，`0% → 5% → 38% → … → 100%`，不再是秒到 100%。
- 当引擎因故没输出 `bytes downloaded` 行时，`written` 保持 0，进度停在 0% 直到进入
  Step 2（校验）/ Step 3（完成）—— 这比误报 100% 安全：用户不会因为假进度而重启。
- 这是显示层修复，不改变写入行为本身；官方包依然正常刷入。

### 验证

- Android release 构建 RC=0（1 m 04 s，无新增 warning）。
- 镜像内二进制 SHA 与本地产物一致（`99b67b3f…e6930aa`）。
- 新 kmsg 标记命中 1 次；`第 1 步，共 3 步：写入新系统`、锁屏、双击亮屏文案仍在。
- `mtp.rc` 存在、`system-init` 缺席、配置为 `mtp=false / mtp_after_decrypt=true / 90Hz / rotate=270 / columns=560`。
- `repack` RC=0。

---

## 2. AVB 三个方案（上次回复被额度中断，这里补全）

背景：TWRP 的 commit `TWRP-Test/android_bootable_recovery@15bb3f70` 只是加一个**编译期** flag，
让已有的 "Disable AVB" 动作同时关掉 `HASHTREE_DISABLED` 和 `VERIFICATION_DISABLED` 两位，
写 `vbmeta_a/b` 两个槽的 vbmeta 头 —— 它**不是新增菜单项**。Skyward 这边已经有底层
`/system/bin/avbctl`（`disable-verity / enable-verity / disable-verification / enable-verification`），
只缺一个 UI 入口。本机只读检查：`verification is enabled on slot _b`、`device_state=unlocked`。

| 方案 | 内容 | 代价 |
|---|---|---|
| **A 不改代码** | 直接用现成 `adb shell avbctl disable-verification` / `enable-verification`，可逆 | 每次要连电脑敲命令；只写当前槽 `_b`，切槽后验证会回来 |
| **B 加高级页按钮**（即你说的「高级 去除 AVB 验证」） | Advanced 加一行「关闭 AVB 验证」+ **滑动二次确认** + 明确警示 + 一键「恢复验证」；复用 `avbctl::set()`，可选两槽都写 | 要写 `vbmeta` 分区头（启动验证链的关键分区）；HyperOS/玄戒可能有额外校验，首次启动有小概率要求清 data |
| **C 只加只读状态** | 高级页显示当前 verity / verification 开关状态，不写盘 | 零风险，但不能改 |

建议：**B**（最贴合「高级里加去除 AVB 验证」），默认只写当前槽、明确警示、滑动二次确认、提供一键恢复。
等你回 A / B / C（或 B 要不要双槽）我再动代码 —— 写 vbmeta 属启动关键分区，不擅自做。

回退镜像：`out/skyward-violin-OS4.0.5-20261116-2301.img`（侧栏拦截版）、
`…20261116-0950.img`（C 方案 MTP 版）、`…20261115-2325.img`（无记住密码版）。
