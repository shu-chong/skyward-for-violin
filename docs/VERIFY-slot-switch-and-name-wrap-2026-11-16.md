# 真机验证：切槽生效 + 安装页包名折行

日期：2026-11-16
镜像：`out/skyward-violin-OS4.0.5-20261116-2346.img`（UI 折行版）
SHA-256：`3fa793af018b505d037d56440ce9650d85eca97b5b97db4cf453ff0d222e9cf5`
上一版（本轮验证切槽所用）：`out/skyward-violin-OS4.0.5-20261116-2322.img`

本轮**未刷机、未写设备分区**；刷机由用户执行，AI 只读抓 log。

---

## 1. 切槽修复：真机验证通过 ✅

用户从 **a 槽**刷官方完整包 `violin-ota_full-OS4.0.6.0.XOTCNXM-user-17.0-e0b0fc6c62.zip`
（8.77 GB），后台实时日志 `.references/logs/live-ota2-20261116-233625.txt`（46,388 行）。

三条关键 kmsg 全部命中：

```
15:36:39  skyward: install: progress now follows the bytes update_engine wrote, of 8770311080
15:42:34  skyward: install: this recovery written back to [B]: Err(write .../recovery_b: Operation not permitted)
15:42:34  skyward: install: Installed { slot: B, wipe: false }
15:42:34  skyward: slot B starts next after the install          ← 切槽成功
```

结论：
- **切槽成功**：`slot B starts next after the install` 出现，说明 `activate_installed_slot()` 正确调用
  `set_active(B)` 并读回确认。刷完官方包后下次启动将进入新系统 B 槽，不再卡 Mi。
- **真实进度生效**：`progress now follows the bytes update_engine wrote` 出现，进度条由引擎真实写入字节驱动。
- **reflash 仍失败**（已知）：`recovery_b: Operation not permitted`，与上一轮相同，是 OTA 结束瞬间平台对
  recovery 分区的保护；系统本身刷入成功（`Installed { slot: B }`）。切槽进系统不受影响。
- 后台 logcat 任务在刷机重启、adb 断开时退出（exit 255），这本身印证设备确实重启切槽了。

### 仍未验证的一点

日志证明"已把活动槽设为 B"，但**是否重启后真的从 B 槽正常进系统**，需要用户重启确认
（看 `ro.boot.slot_suffix` 是否变 `_b`、是否停在 Mi 界面）。这一步由用户完成。

---

## 2. reflash 失败提示：本轮未改

用户再次看到 `未能将 skyward 重新刷到 recovery 分区 / write recovery_b: Operation not permitted`。
这是提示文案，不是故障。**待用户确认是否要把提示改成「请用 fastboot 单独刷 recovery_b」**。

---

## 3. 安装页包名折行 —— 已改

### 问题

原来标题是 `tr!("正在安装 {v}")`，把版本号（如 `17OS4.0.261112.190429467.XRPDCN.S`）
拼进标题。`Canvas::text` 超宽时截断加省略号（`…`），长名字**前后都被砍掉**。

### 改法

按用户给的目标格式重构 `Install::Updating` 页：

```
正在安装
<包名/版本号，按需自动折行，完整显示>
第 1 步，共 3 步：写入新系统 42%
[进度条]
```

- `crates/skyward-ui/src/widgets.rs` 新增 `progress_card_named(title, name, detail, done)`：
  - "正在安装"独立一行；
  - name 用 `text_width` 逐字符测量、按字符硬折行（长版本号/无空格串也能换行且不溢出、不省略）；
  - 下面是进度描述行和进度条，卡片高度随折行行数自适应。
- `crates/skyward-ui/src/pages.rs` 的 `Install::Updating` 改用新控件，标题固定"正在安装"，
  version（无则"系统更新"）作为 name 传入。
- 其它页面用的原 `progress_card` 未动。

### 效果

- "正在安装"单独一行，不再挤占包名空间；
- 长包名完整显示、自动换行，不再缺前少后。

---

## 4. 验证

- Android release 构建 RC=0（1 m 02 s，无新增 warning）；宿主 pack RC=0；打包 RC=0。
- 镜像内二进制 SHA 与本地一致（`72fbfb9e…686f7`）。
- 新文案/标记命中：`正在安装`×3、`系统更新`×7、`starts next after the install`×1、
  `安装官方包约需 5 分钟`×1、`安装完成前，请不要关机`×1。
- `mtp.rc` 存在、`system-init` 缺席、`repack` RC=0。
- 宿主 `cargo test` 仍被本机 `rsproperties` Windows 编译问题挡住（与改动无关）。

## 5. 刷入后验证清单

1. 用本镜像重刷 recovery（`fastboot flash recovery_b <本镜像>`，或当前槽对应的分区）。
2. 再刷一次官方包（或直接看已有 B 槽系统），重启确认 `ro.boot.slot_suffix=_b`、不停在 Mi 界面。
3. 从内部存储安装时，看安装页："正在安装"独占一行、包名完整折行、进度条与百分比正常。
4. 若想进 Skyward：`fastboot flash recovery_b <本镜像>` 后 `fastboot reboot recovery`。

回退镜像：`…20261116-2322.img`（切槽+mod_device 版）、`…20261116-2309.img`（真实进度版）。
