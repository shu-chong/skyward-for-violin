# dijun_skyward（Xiaomi 15S Pro / XRING O1）适配评估

调研日期：2026-11-14 · 全程只读（下载源码、读 diff、写报告），**未改任何源码、配置、镜像，未连设备**。

仓库：<https://github.com/Lw5959/dijun_skyward>
- 单提交 `32946096d5`（2026-11-13 17:22 UTC），`Initial release: Skyward for Dijun v0.2.1`
- 基于 `kirklin/skyward` 的独立分叉，版本 `0.2.1`；**官方主线现已到 `v0.3.1`**（`v0.2.0` → `v0.3.0` → `v0.3.1`），无 `v0.2.1` tag。
- 本机源码副本：`.references/dijun_skyward-main.zip`、`dijun-src/`（35,992,582 字节 / 392 文件）。
- 与我们的 v0.2.0 + violin 补丁逐文件对比：**32 个共有 `.rs` 文件有差异**。

## 1. 同 CPU，但外设并不相同

| | dijun / 15S Pro | violin / Pad 7S Pro |
|---|---|---|
| SoC | XRING O1（玄戒） | XRING O1（玄戒） |
| 基线 ROM | `OS4.0.4.0.XODCNXM` | `OS4.0.5.0.XOTCNXM` |
| 屏幕 | 1440×3200，手机竖屏 | 3200×2136，平板横屏（像素更多） |
| 面板最快档 | **120 Hz**（真机 3 秒 365 次提交 ≈ 120.26 fps） | **144 Hz**（花屏）、90 Hz 正常 |
| 触摸 | Synaptics TCM (`synaptics_tcm2`) | Novatek NT36536 (`nt_touch`) |
| 马达 | CS40L26，有 | **无**（已确认） |
| 蓝牙 | 完整 BlueZ + A2DP | 本构建不支持 |
| WLAN | MT6639 完整方案 | 本构建不支持 |

**关键提醒**：dijun 的 120 Hz 不是「修好了高刷」，而是**它面板最快档本来就是 120 Hz**，代码仍用上游 `max_by_key(vrefresh)`。我们 144 Hz 花屏是 `3200×2136 @144` 需要约 40 GB/s DDR、而 recovery 没有带宽投票（见 9.18）。**照搬他们的显示代码不会让 violin 上 144 Hz。**

---

## 2. 他们到底修了什么（按主题）

### 2.1 息屏 EINVAL：`surface.rs` 完整解绑显示管线 —— **比我们当前的绕过方案更正确**

我们的 log（`logs/20261113-2052`）里是 `dpu_power_check: commit valid plane to off crtc` → `ret -22`；我们 9.14 的做法是把失败降级、把 `power` 留在 `Resting`（**绕过**）。dijun 在 `surface.rs::turn_off()` 里正面修掉：

```rust
if self.lp.is_none() {
    // O1 没有 Qualcomm 的 LP 属性。仅清 ACTIVE 会留下仍绑定的
    // plane/mode 并被驱动拒绝；完整解除这条显示管线后再关闭 CRTC。
    for &(plane, fb_prop) in &self.planes {
        let props = Props::read(&self.card, plane)?;
        req.add_property(plane, fb_prop, property::Value::Framebuffer(None));
        req.add_property(plane, props.handle("CRTC_ID")?, property::Value::CRTC(None));
    }
    let props = Props::read(&self.card, self.conn)?;
    req.add_property(self.conn, props.handle("CRTC_ID")?, property::Value::CRTC(None));
    req.add_property(self.crtc, self.mode_id, property::Value::Blob(0));
}
req.add_property(self.crtc, self.active, property::Value::Boolean(false));
```

唤醒时 `present()` 成对恢复：plane 的 `CRTC_ID`、connector 的 `CRTC_ID`、`ACTIVE=true`、`MODE_ID`。
`lp.is_none()` 是判断有没有高通 LP 属性的开关——**玄戒没有，所以走通用路径**。
真机记录：息屏唤醒 **约 175–220 ms** 亮屏（我们降级方案是 2.4 秒）。

### 2.2 触摸：**没有 PR #30 的每秒重写** —— 独立印证我们 9.20 的诊断

dijun 的 `input.rs` 相对我们 `-99 行`，里面删掉的正是：
- `TOUCH_REFRESH` / `keep_touch_told()` 定时线程（**完全没有**）；
- `swapped` / `reversed` 坐标映射（手机竖屏原生，不需要）。

同平台另一位适配者也没用这个每秒写 sysfs 的做法。我们已实测它在 NT36536 上每秒触发一次
`nvt_xm_htc_set_idle_baseline_update`，导致长按滑动断触。**我们计划中的「读回比较 + 有手指推迟」是对的方向；也可以直接照 dijun 删掉定时线程。**

另外它把 `eprintln!(touch ...)` 改成 `skyward_sys::kmsg`（init 阶段 stderr 指向 `/dev/null`），并把面板就绪时间/量程写进正式日志——这个诊断改进值得抄。

### 2.3 双击亮屏：`touch.rs`（新增）+ `KEY_WAKEUP` —— 对应我们的「息屏后触摸不亮屏」

- `input.rs`：`KEYS` 加 `KeyCode::KEY_WAKEUP`，`Event` 加 `Wake` 变体；
- 新增 `crates/skyward/src/touch.rs`：`/dev/xiaomi-touch` ioctl，先 `SELECT_TOUCH_ID(0)`
  （`((b'T'<<8)|3, 0)`），再设 **Touch_Doubletap_Mode = 14**，结构体 1032 字节、
  `3<<30 | SIZE<<16 | 'T'<<8`，**设置后回读校验**；
- `ui.rs`：`Event::Wake` → `app.wake_gesture(now)`，设置页开关 `Effect::DoubleTapWake` 可保存；
- 配置 `[touch] xiaomi_double_tap = "/dev/xiaomi-touch"`。

真机记录：两次物理 `Double TAP Detected` → `KEY_WAKEUP` → UI 恢复。
这正是我们 9.17 里 `App::touch()` 暗屏直接 `return Vec::new()` 想绕过的那条路——
**用原厂低功耗手势，不需要让息屏时触摸控制器继续全功率扫描**，比我们原设想的
「`wake=true` + 跳过 rest/turn_off + 禁止 may_sleep」干净得多。

### 2.4 `logd` 崩溃循环：打包期改 `logd.rc` —— 直接解我们的 9.19 D 项

`skyward-pack/src/main.rs:136-142`（仅 `codename == "dijun"` 分支）：

```rust
let name = "system/etc/init/logd.rc";
ensure!(text.matches("    priority 10\n").count() == 1, "...");
rd.replace(name, text.replacen(
    "    priority 10\n",
    "    priority -2\n    rlimit nice 22 22\n", 1).into_bytes())?;
```

注释：*本机前台策略要求 nice=-2；init 在 setuid 后才设置 priority。仅给 logd 的
RLIMIT_NICE 放行到 -2，避免授予整个 SYS_NICE 能力。*

我们的 `failed to set forground cgroup: Permission denied` → `SIGABRT` → 每 5 秒重启
（30 次 / 255 s）正是这条。`ro.logd.kernel` 只读冲突只是伴随 warning。
另外 `main.rs::start_logd()` 会把 `[logd].libraries` 从已安装系统复制到
`/system/lib64`，再 `ctl.start logd` + `wait_prop("logd.ready","true",5s)`，
**不是让 init 无限重启**。我们目前只有配置里的 `[logd] libraries`，没有这一步。

### 2.5 fstab：`preserve_o1_data()` —— **「不损坏设备」的加固，我们完全没有**

`fstab.rs` 固定 `FILES[3]` 增加 `/vendor/etc/fstab.O1_asic`（我们 9.3 是动态扫描
`fstab.*`，功能等价、更通用）。真正值得抄的是读表后的处理：

- 删掉 `/data` `/metadata` `/mnt/vendor/persist` `/mnt/vendor/nvdata` 的
  `check` / `formattable` / `resize` flags → **解密入口不自动修复或格式化**；
- persist、nvdata 改 **`ro,noload`**，并去掉 `noauto_da_alloc` / `nodelalloc` / `commit=`
  这些与只读 noload 冲突的写挂载调优；
- 移除 `rescue → /cache` 映射（原厂 recovery 把它当 cache，**它不是可清空的缓存分区**）；
- 保留原厂 FBE v2 / `wrappedkey_v0` / metadata encryption 参数不变；
- 打包器里也对 `system/etc/recovery.fstab` 做同样处理。

这是同平台适配者专门加的数据保全逻辑，和我们「所有操作以不损坏设备为前提」的约束完全一致。

### 2.6 模块加载器：`module_graph()` 支持 O1 版本目录

`skyward-modules/src/lib.rs` 新增：从实际找到的 `.ko` 向上找**所属版本目录**里的
`modules.dep`（`/lib/modules/<内核版本>/extra/...`），而不是假定平铺目录；
`system_dlkm` 也按 `/system_dlkm/lib/modules` 优先。带 2 个单元测试。
配置 `[modules] sources = ["vendor_boot","vendor_dlkm","vendor","system_dlkm"]`、
`firmware_directories`。

我们目前 `[modules].load` 只列 4 个平铺的触摸模块且真机能加载，所以**暂不是阻塞项**；
但以后若要加载 `xring-drm` 等版本目录内模块，这是必需改动。

### 2.7 WLAN 完整启动链 —— 解掉我们 9.13 复盘的「时序」问题

`devices/dijun.toml` 给出官方顺序的等价实现（我们之前一直卡在 `pre_cal_flow:0`）：

| 环节 | dijun |
|---|---|
| 开机模块 | `[modules].load` 就含 `conninfra`、`rfkill`、`wlan_drv_gen4m_eap_6639`（**不是点 Wi-Fi 才加载**） |
| 初始化 | `conninfra_loader` oneshot → `setprop vendor.mtk.nvram.ready 1` → **`uart_launcher -f 3 -p /dev/ttyXR0`** → `wlan_assistant` |
| 注释关键 | 「UART 与 BT 回调在用户开启 WLAN **前**注册，**共享预校准才能完成**」 |
| 固件 | 打包器写 `system/etc/firmware/conninfra.cfg` = `co_clock_flag=1`；「mode=2 会让 BT_open 一直等 is_pre_cal_done」 |
| 节点权限 | `/dev/wmtWifi` → `wifi:system 0660`；`/dev/conninfra_dev` → `system:system 0660` |
| 开关 | `[wlan] writes=/dev/wmtWifi "1"`、`off_writes="0"`（**关闭时交还电源控制**） |
| 上电 | `wlan.rs::bring_up()` 每次显式写 `wmtWifi=1`，再等内核真正 `IFF_UP`，不把关机残留的 `wlan0` 当可用 |
| 已存网络 | 从 Android 导入 13 条保存网络、重连优先、开关状态记忆 |

我们 9.13 复盘的结论（把启动链压到开机几秒内连续完成）在这里得到真机验证；
`uart_launcher` + `nvram.ready` + `co_clock_flag=1` 是我们当时**完全没做**的三件事，
正好对应 `wlan_func_on_by_chrdev Timeout, pre_cal_flow:0`。

### 2.8 打包器安全加固

`pack/main.rs` + `pack/avb.rs`：核对**整份**原厂 Recovery 的 SHA-256 与大小、
`StockMetadata::read()` 继承原厂 **AVB rollback index / location / 属性描述**、
`footed_with_stock()` 重算摘要而不沿用原签名；输出 ZIP/JSON/SHA256 供更新器。
构建脚本 `tools/build-dijun.sh` 里也硬校验 stock 哈希。

另有 **KernelSU 模块**发布方式（不刷分区，KSU 里切换 recovery，卸载即还原），
对「不损坏设备」很友好，但我们已用 `fastboot flash recovery_b` 多轮且两槽健康，
当前不必改。

### 2.9 其它（本构建不适用）

- `[haptics] sine = { period_ms=8, magnitude=255 }`：CS40L26 标准 FF_SINE，有限时长、结束删效果。**violin 无马达**。
- 蓝牙 `bluetooth = true` + `skyward-bluetooth` crate + BlueZ/BlueALSA：本构建不需要。
- `[display] brightness = { normal = 5460, full = 16383 }` + `normal_max()`：
  真机 `bl_normal_max=5460`，「不套用 msm_drm 的面板查找入口」。我们是 `normal_max()` 读不到
  `msm_drm` 参数就回退 `max_brightness`，**可参考但需实测 violin 的 HBM 门限**。
- `[update] server`、Xiaomi/GitHub 更新查询、`skyward-vab/partitions.rs`：非当前优先级。
- `services.rs`：dijun **没有**我们的 `optional` 容错（他们的服务都能起来）；我们的保留。
- `bootimg/ramdisk.rs`：他们 `-33 行`，版本基线不同，**不要覆盖**我们为 Windows 宿主做的 `#[cfg(unix)]` 隔离。

---

## 3. 可用性分级（针对 violin 0.2.0 + 4.0.5）

### P0 —— 建议下一轮直接合入（有真机证据、与当前待修项一一对应）

| # | 项目 | 出处 | 解决我们的问题 |
|---|---|---|---|
| 1 | 删掉 `keep_touch_told()` 定时线程（或改读回比较 + 有手指推迟） | `input.rs` / 9.20 | 长按滑动断触、密码按不动 |
| 2 | `surface.rs` 关 CRTC 前完整解绑 plane/connector，唤醒成对恢复 | `surface.rs` | 息屏 EINVAL、2.4 s 延迟、进程崩溃（**比我们的 Resting 绕过更正确**） |
| 3 | 打包器把 `logd.rc` 的 `priority 10` → `priority -2` + `rlimit nice 22 22` | `pack/main.rs` | logd 每 5 s SIGABRT、dmesg 刷屏、logcat 不可用 |
| 4 | `preserve_o1_data()` 的等价加固 | `fstab.rs` | 解密入口自动 fsck/格式化风险、persist/nvdata 写挂载、rescue 误当 cache |
| 5 | 主配置 `refresh` 固化 90 | 我们 9.19 | 90 Hz 已真机验证正常 |

### P1 —— 值得做，但需单独验证

| 项目 | 说明 |
|---|---|
| 双击亮屏 `touch.rs` + `KEY_WAKEUP` + `xiaomi_double_tap` | `/dev/xiaomi-touch` 接口通用，但 **NT36536 是否支持 mode 14 未验证**；先只读 ioctl 回读探测，别直接写 |
| `module_graph()` 版本目录 `modules.dep` | 目前不阻塞；以后加载 `xring-drm` 等模块时必需 |
| 打包器 stock SHA / AVB rollback 继承 | 安全加固，零功能风险；但会硬编码 4.0.5 stock 哈希（我们已有 `376631cf…30293`） |
| `[display] brightness.normal/full` | 需先实测 violin 的 `bl_normal_max` |
| 触摸就绪信息写 kmsg 替代 eprintln | 诊断改进，零风险 |

### P2 —— 以后修 WLAN 时整套参考

`2.7` 的整条链（开机模块 → `conninfra_loader` → `nvram.ready` → `uart_launcher` →
`wlan_assistant` → `wmtWifi=1` → 等 `IFF_UP`）+ `conninfra.cfg co_clock_flag=1`。
**本轮仍按用户决定「不支持 WLAN」，不动。**

### 不适用

振动（无马达）、蓝牙、KSU 发布方式、`[update]`、`services.rs` 删 optional、
`ramdisk.rs` 覆盖、以及「照搬显示代码上 144 Hz」（见第 1 节）。

---

## 4. 与我们当前待办的关系

用户 2026-11-13 21:23 说「修复」，指的是上一轮商定的 **1+2+D+E**；该轮被中断，
**代码一行未改**（`git status` 干净，最后提交 `afb866a` 只是文档）。

本次调研给出了更优解，建议把方案升级为：

- 触摸：**直接删定时线程**（dijun 同平台也没用），比「读回比较 + finger_status」更简单、零新增配置字段；
- 息屏：**用 dijun 的完整解绑/恢复**替换我们的 `Resting` 降级（`is_off()` 也恢复上游只判 `Off`）；
- logd：**用 dijun 的 `logd.rc` 打包期修正**，比我们原来「未定位根因」的 D 项有明确方案；
- 90 Hz：主配置固化；
- 可选顺带：`preserve_o1_data` 加固（数据保全，用户一贯要求）。

**注意版本差异**：dijun 是 `0.2.1` 分叉且含蓝牙 crate，不能整仓合并；
上述补丁都需按我们 `v0.2.0` 的代码位置手工移植。官方主线已有 `v0.3.0` / `v0.3.1`，
是否升基线应单独决策，不要和本轮修复混在一起。

## 5. 复核方式

- 源码副本：`.references/dijun-src/`，可随时 `diff -u crates/... .references/dijun-src/crates/...` 复核；
- 官方说明：`README.dijun.md`、`docs/dijun-validation.md`（真机功能矩阵含未验收项，作者标注得很诚实）；
- 设备配置：`devices/dijun.toml`；构建入口 `tools/build-dijun.sh`。
