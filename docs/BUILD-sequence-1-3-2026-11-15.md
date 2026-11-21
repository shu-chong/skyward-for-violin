# 按 onyx 评估的顺序推进：第 ①②③ 项

日期：2026-11-15 20:58
镜像：`out/skyward-violin-OS4.0.5-20261115-2058.img`
SHA-256：`c224d747fb0e0d3c8137d7cf199efa81e8f4f38f06872f887ced26c472322244`
大小：104,857,600 字节（ramdisk 49,170,514），生成于 20:58:37
依据：`docs/REVIEW-onyx-upstream-2026-11-15.md` 第 5 节的建议顺序

本轮**没有刷机、没有写入任何设备分区**。承接上一版 `…2036.img`（只跑 adb）的全部改动。

## 本轮做了什么

| 顺序 | 内容 | 状态 |
|---|---|---|
| ① | 本地 `recovery.img` 更新 + 解密完成倒计时 | ✅ |
| ② | `wait_file()`：模块节点就绪再 hand over | ✅（容错版，见下） |
| ③ | WLAN `iface::set_up` 的 EBUSY 重试 | ✅ |
| ④ | 双击亮屏 | ⏳ 下一轮（要改息屏策略，单独验证） |
| ⑤ | 锁屏 | ⏳ 与 ④ 一起 |
| ⑥ | 记住解锁密码 | ⏸ 等安全决策 |
| ⑦ | WLAN 自动连接 / 在线更新 | ⏳ 最后 |

## ①-A 从本地 `recovery.img` 更新

上游 `v0.3.1` 本来就有 `crates/skyward/src/updates.rs::write()`：
`upgrade::unzip` → `upgrade::check` → `upgrade::slots` → `upgrade::write`。
`upgrade::check` 从镜像里读 `skyward/device.toml` 并核对 codename，**裸 `recovery.img` 也能过校验**，
所以不需要 zip、不需要网络。

本轮改动：

- `updates.rs`：新增 `Task::UpdateSkywardFile { path, codename }`；`write()` 拆成
  `write_image(read, ...)`，新增 `write_raw()`（把 `upgrade::unzip(path)` 换成 `fs::read(path)`，
  其余复用）。Worker 直接 `tell(Event::Ended(write_raw(...)))`，进度/结果走原有的
  `skyward_progress` / `skyward_ended`，页面表现和在线更新一致。
- `skyward-ui`：`Action::UpdateSkywardFile` / `Effect::UpdateSkywardFile`；
  `act.rs` 复用「安装空闲、没有更新在进行」的检查并建 `SkywardJob`。
- `pages.rs::skyward_update()`：新增「本地更新」段，一行按钮
  「从本地 recovery.img 更新」，副标题写明路径。
- `ui.rs`：固定路径 `/sdcard/Download/recovery.img`（与 onyx 一致）。
  **文件不存在、已有更新在跑、Worker 启动失败都改为 `SkywardEnd::NotWritten` 报告，
  不再 `?` 冒泡** —— `effect()` 里的 `?` 会一路冒泡到 `run()` 杀掉 recovery，
  这正是我们前面几轮一直在避免的。

**用法**：把新镜像 `adb push` 到 `/sdcard/Download/recovery.img`（需先解密 data），
进「高级 → Skyward 更新 → 本地更新」点按钮即可，不用连电脑 fastboot。

**安全边界**：`upgrade::slots()` 会写入**下次启动的槽位**，以及另一个「当前装着 Skyward」
的槽位；只要 `recovery_a` 仍保留原厂镜像，就只会写 `recovery_b`。
`upgrade::check` 校验镜像内的 codename 必须是 `violin`，防止刷进别的机型的镜像。
仍然只写 recovery 分区，不碰 boot / bootloader / 分区表。

## ①-B 解密完成倒计时

- `State.home_at: Option<u64>`；`App::tick()` 在解密页且 `Data::Open/Erased` 时
  设 `now + 3000`，到点 `select_section(Screen::Home)` 自动回主界面；
  点「完成」（`Action::Skip`）离开页面时自动清除，不会误跳。
- `App::deadline()` 把 `home_at` 纳入唤醒时间，主循环到点会重绘。
- 解密页按钮文案改为「完成 · 3 秒后自动返回」（静态文案，不逐秒刷新）。

## ② `wait_file()`（官方 PR #56，容错加强版）

- `skyward-sys` 新增 `wait_file(path, timeout)`（等模块创建的节点出现）。
- `main.rs::hand_over()` 在 chown 之前等待 5 秒；**节点始终没出现、chown 失败、
  chmod 失败都只写一条 kmsg 并跳过该节点**，不再让单个节点终止 recovery。
  这比 PR #56 原版（超时即报错）更适合我们的场景。

**没有新增 `[[owners]]` 条目**：BT 节点权限当初被放弃，正是因为 `hand_over` 是硬失败；
现在机制已经安全，但 WLAN 已在真机上正常工作，加 owners 属于无收益的额外变量。
以后若某个设备节点确实需要特定属主，直接往 `violin.toml` 加即可，不会再开不了机。

## ③ WLAN 的 EBUSY 重试

`wlan.rs::bring_up()` 末尾：`wlan0` 接口会先于驱动就绪出现，`iface::set_up`
在此期间返回 EBUSY。改为最多 10 秒、每 200 ms 重试，期间写一条 kmsg；
最终仍失败才返回带上下文的错误。取自 onyx `0003` 补丁，零风险。

## 构建与自检

| 项 | 结果 |
|---|---|
| 宿主 `skyward-pack` | RC=0 |
| Android `skyward` | RC=0，8,816,656 字节 |
| 配置校验 `stock --config` | RC=0 |
| `skyward-mtp.rc` | 缺席（保持只跑 adb） |
| `skyward/system-init` | 缺席（无双系统） |
| `prop.default` | `ro.adb.secure=0`、`persist.sys.usb.config=adb` |
| 镜像内配置 | `mtp=false`、`supports_vibration=false`、`refresh=90`、`rotate=270`、`columns=560` |
| 新代码标记 | 「从本地 recovery.img 更新」「3 秒后自动返回」「the WLAN interface is not ready yet」「going on without it」均在二进制内 |
| `logd.rc` / `conninfra.cfg` | `priority -2` + `rlimit nice 22 22` / `co_clock_flag=1` |
| ramdisk 往返 | 1048 条一致 |
| 二进制一致性 | 镜像内 `system/bin/recovery` 与本地产物同为 8,816,656 字节 |

stock 底稿仍为 `D:\DNA_WIN-230828\…\001_DNA\recovery.img`（`376631cf…30293`）。
中途打过一版 `…2056.img`，因发现 `effect()` 的 `?` 会杀进程已删除，以 `…2058.img` 为准。

## 刷入后请重点验证

1. 「高级 → Skyward 更新」出现「本地更新」段；先**不放文件**点一次，
   应只在页面上提示找不到文件，**不能黑屏 / 不能重新解密 / adb 不能断**。
2. 放一个本机型镜像到 `/sdcard/Download/recovery.img` 再点，看写入进度与结果；
   注意它按 `upgrade::slots()` 决定写哪些槽位。
3. 解密完成后按钮显示「完成 · 3 秒后自动返回」，3 秒自动回首页；点「完成」立即走。
4. 开机日志里 adbd 仍只启动 1 次、无 `xr_usb_composite_disconnect`；
   90Hz、WLAN、触摸、旋转、充电动画无回归。

回退：`out/skyward-violin-OS4.0.5-20261115-2036.img`（`fafe86d9…da5`）或原厂底稿。
只刷 `recovery_b`，`fastboot reboot recovery` 本机实测可用。

## 下一轮

④ 双击亮屏需要把息屏策略改成「只关背光、面板保持 On、输入继续供电」，
与当前 dijun 完整解绑路径冲突，且 NVT 触摸息屏会 suspend —— 单独一轮、默认关、
真机验证后再接 ⑤ 锁屏（必须画在整屏 `whole`）。
⑥ 记住解锁密码仍等你在「不做 / 默认关+明文警示+一键清除 / 改存密钥材料」中选择。
