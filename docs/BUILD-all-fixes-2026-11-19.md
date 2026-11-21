# 全代码审查修复版：构建与交付

日期：2026-11-19 09:19
镜像：`out/skyward-violin-OS4.0.5-20261119-0919.img`
SHA-256：`31e0a47f6b3bcc0a719912d8bb5c3f316fd236687e4108ea992e39bd440293b8`
基线：审查报告 `docs/CODE-AUDIT-2026-11-19.md`

本轮把审查报告里 S1-S3、M1-M9、L1-L12 全部修掉并构建。**未刷机、未写设备分区。**

## 修复清单（按审查报告编号）

### 严重
- **S1 自动合并竞态致 recovery 退出**：`ui.rs` begin() 起 merger 后设 `Standing::Merging(0.0)`
  （用 if/else 三分支各自 set_update，避免被覆盖）；`Effect::Merge` 改幂等——已有 merger 只写 kmsg
  并 `return Ok(Vec::new())`，不再 `ensure!` 冒泡致命错误。两道保险。
- **S2 MTP 切换失败丢 adb**：`enable_mtp_after_decrypt()` 和 `file_transfer()` 都改为
  `usb::switch` 成功才 `transfer=Some(t)`；失败即 `t.stop(true)` 回滚到 adb、清 `file_transfer` 标志并报页面，
  不再把端口留在 `none` 且本会话不可自愈。
- **S3 自动合并取舍**：用户拍板保留（"已刷机自然要抹掉旧系统"）。补 kmsg
  `merging is irreversible: the system before the update cannot come back` 明确告知。

### 中等
- **M1** `navigation_open()` 加入 `Install::Wiping`，侧栏不再能在清数据中切页。
- **M2** 息屏且双击开启时绕过 `tablet.route()`，用 `route_whole()` 把坐标交给 `app.touch()`，
  平板左侧 560px 导航列区域也能双击亮屏。
- **M3** 写回失败提示改为："系统已安装成功、不影响开机 + 请用 `fastboot flash recovery_b <镜像>` 恢复 Skyward"，
  不再让用户误判为刷机失败而重刷。
- **M4** `reflash()` 加 4 次 ×750ms 重试，仅对瞬时 EPERM（os error 1 / Operation not permitted）重试，
  其它错误立即失败；与 BLKROSET protection 叠加。
- **M5/M6** 自动 MTP 改由 `turn()` 每帧按 `data.storage_open()` 触发（幂等），去掉 `closed.is_empty()` 判据；
  `decrypt_told()` 里的重复触发移除——不走解锁流程也会自动开 MTP。
- **M7** `conninfra.cfg` 改为存在性判断：已有则只补 `co_clock_flag` 行、保留原配置，无则新建，打包不再因 stock 变化失败。
- **M8** `logd.rc` 未命中预期行时打印构建警告，不再静默 no-op。
- **M9** WLAN 设 up 只对 EBUSY（errno 16，经 `downcast_ref::<io::Error>()`）重试 10s，其它错误立即返回。

### 轻微
- 删 `wake_lock` 死状态（恒真条件）、`State.wlan_password` 死字段、未用配置 `DeviceConfig.bluetooth` /
  `Touch.xiaomi_double_tap`、`Mount::served()` 死代码及其 atomic import。
- 清孤儿文档注释（`decrypt.rs`/`settings.rs`）、改正过期注释（`skyward/src/settings.rs`、`violin.toml`）。
- settings 文件改用 Unix `OpenOptions::mode(0o600)` + `DirBuilder::mode(0o700)` 从创建起就私有，
  无 0644 明文窗口（原来是 create 后 chmod）。
- `Keeper::new` 对 `Unreadable` 按 `Nothing` 处理，允许默认值落盘，不再整会话不写。
- 锁屏页 `Frame { back: false }`，去掉点了没反应的返回箭头与边缘滑动。
- `disable_both()` 的 locked 检查同时看 `ro.boot.vbmeta.device_state` 和 `androidboot.flash.locked`，
  属性缺失不再算 unlocked。
- 补单测 `the_side_column_is_shut_while_every_kind_of_install_is_busy`，覆盖全部忙/闲 Install 变体，
  可直接捕获 M1 这类漏项（宿主测试因已知 `rsproperties` Windows 平台问题无法在此机跑，语法已逐项核对）。

## 构建与自检

- Android release 构建 RC=0；宿主打包器 RC=0。全程无新增 warning（仅宿主 bootimg 4 个 cfg 桩 warning，与改动无关）。
- stock 底稿：OS4.0.5 解包目录已不在 D 盘，用 `skyward-pack stock` 从配置 URL 提取，
  SHA-256 `376631cf…` 校验通过，与之前镜像同一底稿。
- 镜像内二进制 SHA 与本地产物一致（`cb9ead1b…eabc`）。
- 新标记全部命中：自动合并不可逆告知、合并幂等、MTP 回滚、写回失败新提示。
- 配置不变：`mtp=false` / `mtp_after_decrypt=true` / 90Hz / rotate270 / columns=560；
  `skyward-mtp.rc` 在、`system-init` 不在、`prop.default` 仍 adb-only。
- `repack` 往返 RC=0（49,197,056 B）。

## 真机验证清单

1. 刷这版进 recovery，确认双击亮屏、锁屏右滑、WLAN、90Hz、触摸无回归。
2. 刷官方包 → 完成页写回失败时显示新提示（系统成功、fastboot 指引）；若写回成功仍显示"已刷入 recovery"。
3. 卡 Mi 后进 recovery：应自动合并（kmsg `merging it now` + `merging is irreversible`），
   且合并中点"滑动合并"不再让 recovery 退出。
4. 解密后 MTP：从任意路径（含不走解锁页）data 可读即自动启用；若切换失败自动回 adb。
5. 清数据（rescue）中侧栏不可切页。

## 回退

`out/skyward-violin-OS4.0.5-20261117-2304.img`（审查前版本）。
