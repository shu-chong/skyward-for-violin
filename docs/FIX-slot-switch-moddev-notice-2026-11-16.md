# 修复：刷官方包后卡 Mi 重启 + mod_device 报错 + 刷机提示

日期：2026-11-16
工作区：`C:\Users\shc\Desktop\skyward-violin`（旧 `skyward` 目录只读）
镜像：`out/skyward-violin-OS4.0.5-20261116-2322.img`
SHA-256：`cf877a86f69d0dbc36ce8a95e96ce3ea454830e3c46bd1c5fb8011c9ee5a3679`

本轮**未刷机、未写任何设备分区**。

---

## 1. 刷完官方包卡 Mi 界面重启 —— 已修（根因：没切活动槽）

### 你的关键线索

> 安装 yarp 后，进入重启界面，切换槽位，就可以正常开机了。

这一句直接定位了根因：**系统本身刷成功了，只是 bootloader 的活动槽没切过去**。

### 根因（代码 + 日志双重确认）

- 上一轮 OTA 日志铁证：`update_engine: Update successfully applied`、`Installed { slot: B }`。
  官方包完整写进了 **B 槽**，update_engine 报成功。
- 但 Skyward 的安装成功处理 `update_system()`（`crates/skyward-install/src/run.rs`）里，
  成功后**只调用 `reflash()` 把自身写回 recovery，没有任何 `set_active()` / 切活动槽调用**。
- update_engine 在 sideload 模式下只把 payload 写进非活动槽、标记它为可启动，
  **不会**替 recovery 把活动槽切过去。于是重 bootloader 仍从旧槽（A）启动；
  旧槽此时可能已被写坏或是半套系统 → 卡在 Mi 界面反复重启。
- 你在 yarp 里手动切到 B 槽就能开机，正好印证：缺的就是这一次切槽。

### 改法

`crates/skyward/src/ui.rs`：

1. 新增 `activate_installed_slot(slot)`：打开 boot control（复用 `set_slot` 的 binder/hardware 通路），
   `control.set_active(slot)`，再 `control.active()` 读回确认，写 kmsg
   `slot X starts next after the install`。不碰双系统的 boot set（普通 OTA 与之无关）。
2. 在 `installer_told()` 的安装完成块里，收到 `Outcome::Installed { slot, .. }` 时调用它，
   **放在 wipe/cache 处理之前**，这样连"安装后擦除 data"的包也会先进新槽。
3. 切槽失败只写 kmsg（`could not make slot X start next: …`），不中断安装 —— 安装本身已成功，
   用户仍可在重启页手动选槽。

效果：刷完官方包后，下次重启直接进入刚写好的新槽系统，不再卡 Mi。

**注意**：上一轮日志显示写回 `recovery_b` 曾报 `Operation not permitted`（OTA 结束时分区保护），
所以新槽的 recovery 分区可能仍是官方 recovery 而非 Skyward。切槽进系统不受影响；
但如果你之后还想进 Skyward，需要用 `fastboot flash recovery_b <本镜像>` 再刷一次 recovery。
（这与切槽修复是两件事，本轮未改 reflash 行为。）

---

## 2. 「检查更新失败：the installed system says no ro.product.mod_device」—— 已修

### 根因

`installed()`（`crates/skyward/src/ui.rs`）硬性要求系统分区里有 `ro.product.mod_device`；
刷入的 OS4.0.6.0 包不再提供这个 prop，于是 `prop()` 闭包报错，在线更新检查直接失败。

### 改法

`device` 字段改为：`ro.product.mod_device` **优先**，缺失时回退到 `ro.product.device`。
两者都缺才报错。这正好契合 `skyward-net::Installed::region()` 的既有逻辑
（无地区后缀 = CN），所以中国版系统的请求参数不变。

效果：OS4.0.6.0（及任何不带 mod_device 的官方包）都能正常检查更新。

---

## 3. 刷机过程的提示文案 —— 已加（润色版）

位置：`crates/skyward-ui/src/pages.rs` 的 `Install::Updating` 页，接在原有
"安装完成前不要关机" 提示之后，仅 OTA 系统更新时显示。三语文案：

- **简中**：安装官方包约需 5 分钟，进度长时间不动属正常。显示安装成功前，请勿重启或断电，否则可能无法开机。
- **繁中**：安裝官方套件約需 5 分鐘，進度長時間不動屬正常。顯示安裝成功前，請勿重新啟動或斷電，否則可能無法開機。
- **English**：An official package takes about 5 minutes, so a progress bar that hardly moves is normal. Do not reboot or cut power until it says the install succeeded, or the phone may not start.

润色说明：把"5 分钟"和"进度不动是正常的"放前面消除焦虑，把"禁止重启/断电"和后果
"可能无法开机"放后面强调安全点——避免用户看到上一轮修好的真实进度条走得慢就误判卡死而强 reboot。

---

## 4. 验证

- Android release 构建 RC=0（1 m 01 s，无新增 warning）；宿主 pack RC=0；打包 RC=0。
- 镜像内二进制 SHA 与本地一致（`0849111f…90036`）。
- 新标记命中：`could not make slot`=1（切槽函数已编入）、`安装官方包约需 5 分钟`=1、
  `ro.product.mod_device or ro.product.device`=1、`progress now follows the bytes`=1。
- `mtp.rc` 存在、`system-init` 缺席、配置仍 `mtp=false / mtp_after_decrypt=true / 90Hz / rotate=270 / columns=560`。
- `repack` RC=0。
- 宿主 `cargo test` 仍被本机 `rsproperties` 的 Windows 编译问题挡住（与改动无关），
  改由 Android release 构建保证语法与类型正确。

## 5. 刷入后验证清单

1. 在 Skyward 里刷官方完整包，安装页应出现新的 5 分钟提示。
2. 安装完成后看 log：`slot B starts next after the install`（或 A，取决于当前槽）。
3. 直接重启，应进入刚刷好的新系统、**不再卡 Mi**。
4. 回系统后打开「检查更新」，应不再报 mod_device 错误。
5. 若之后要进 Skyward：`fastboot flash recovery_b <本镜像>` 后 `fastboot reboot recovery`。

回退镜像：`…20261116-2309.img`（`a889bc87…b58`，真实进度版）、
`…20261116-2301.img`（侧栏拦截版）、`…20261116-0950.img`（C 方案 MTP 版）。
