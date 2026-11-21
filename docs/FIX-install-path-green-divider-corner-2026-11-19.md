# 安装页显示包路径（绿字）、分栏圆角闪烁、version/android 保留

日期：2026-11-19 22:48
镜像：`out/skyward-violin-OS4.0.5-20261119-2248.img`

## 1. 版本号下面显示包文件路径（绿色，最多折 3 行）

用户要的效果（给了两种）：

```
/sdcard/Documents/violin-ota_full-OS4.0.6.0.XOTCNXM-user-17.0-e0b0fc6c62.zip
```

```
/sdcard/Documents/
violin-ota_full-OS4.0.6.0.XOTCNXM-user-17.0-e0b0fc6c62.zip
```

### 来源

路径就是安装线程实际打开的那个文件：`Effect::InstallFile(place, path)` →
`in_place(place, &path)` → `Installer::open(path)`。所以内存储里选的文件显示
`/sdcard/...`，U 盘上的显示 `/usb_drive/...`。**adb sideload 没有文件名**（那是
电脑推过来的一个流），此时不显示这一行，卡片与以前完全一样。

### 改动

- `crates/skyward-install/src/lib.rs`：
  `Event::Installing { version, path }`，`path` 为 `Option<String>`。
- `crates/skyward-install/src/run.rs`：
  `Context` 增加 `path`；`Installer::spawn` 增加同名参数；
  `Installer::open` 传 `path.display().to_string()`，`asked(Asked::File)` 同；
  `start`/`rescue`/`Asked::Mapped` 传 `None`（映射出来的块设备不是文件）；
  `update_system()` 发事件时带上 `ctx.path.clone()`。
- `crates/skyward-ui/src/app/install.rs`：
  `Install::Updating { version, path, stage, overwriting }`。
  `act.rs` 在滑动确认那一刻的乐观切换先填 `path: None`，随后的 `Installing`
  事件立刻补上真实路径。
- `crates/skyward-ui/src/widgets.rs`：
  `progress_card_named(..., package: Option<&str>, ...)`，新增
  `wrap_path(c, face, text, px, width, most)`：

  1. 整行放得下 → 1 行；
  2. 放不下 → 在**放得下的最后一个 `/`** 之后断开（所以是“文件夹 / 文件名”两行）；
  3. 文件名自己还超宽 → 再按字符硬断，最多 3 行，最后一行由 `Canvas::text`
     自己省略。`most = 3` 在 violin 上实际只用到 2 行。

  行高 `sub_px × 1.3`，颜色 `pal.good`，起点在版本名最后一行基线之后 1dp，
  详情行、进度条、卡片高度依次顺延。
- `crates/skyward-ui/src/theme.rs`：`Palette.good`
  浅色 `#0B8A3E`（白卡上对比度约 4.6:1，直接用小米绿 #1DCD3A 太浅读不清），
  深色 `#46D170`。

## 2. 平板分栏上下那个圆角闪一下（灰→白）

### 根因

平板是“左侧导航栏 + 右侧页面”两栏：页面画在 `Tablet.detail` 上，它自己的
x=0 在**屏幕中间**（分栏线），不是屏幕左边缘。而翻页动画
`Canvas::slide(..., radius)` 会把“前一页的左上下角”按屏幕圆角磨圆
（`App.corners = cfg.display.corner_radius = 60`）。于是每翻一次页，分栏线的
顶部和底部就各被啃掉一个 60px 的圆角，圆角里露出的是被压暗的底下那页（灰），
动画结束又变回页面底色（白）—— 就是看到的那一下灰闪。

### 改动

`crates/skyward/src/ui.rs`：

```rust
if cfg.display.columns == 0 {
    app.set_screen_corners(cfg.display.corner_radius as f32);
}
```

只有单栏（`columns = 0`，页面真的贴着屏幕左边缘）才磨圆；平板两栏不磨，
`corners` 保持 0，`slide` 里 `front_cover()` 直接返回 1，不再有圆角缺口。
面板本身的物理圆角由屏幕遮住，不需要软件再画。

## 3. version / android 保留（“救回来”）

这两个不是新东西，一直都在，只是**不显示在页面上**：

- `skyward-net::release::Installed { device, version, android, unlocked }` ——
  检查更新时发给小米更新服务器的四个参数：
  `version` = `ro.mi.os.version.incremental`（violin 的 `os_version_prop`，
  例如 `17OS4.0.261112.190429467.XRPDCN.S`），`android` =
  `ro.build.version.release`（例如 `17`）；
- 同一份属性也是**关于页**“OS 版本 / Android 版本”两行的来源
  （`DeviceInfo.os_version` / `DeviceInfo.android`）。

它们都来自 `installed_props(current_slot)`，也就是**挂着的分区里的 build.prop**。
装包时 `detach()` 会把当前槽位的分区全部卸掉，装完不挂回去，于是下一次
`collect()`（readings 线程每隔一段时间全量读一次）拿到的就是空的：关于页那两行
消失。刚进 Skyward 时分区是挂着的，所以“检查更新”一直是好的 —— 按用户要求，
`installed()`（检查更新）这一轮**没有改动**。

### 改动

`crates/skyward-info/src/lib.rs`：新增 `PROPS_KEPT: Mutex<Option<(Slot, HashMap<..>)>>`
与 `kept_props(slot, props)`：

- 读到的属性里有 `ro.build.version.release` → 这是真的系统属性，记下来并返回；
- 没有（分区被卸了，只剩 `ro.boot.*`）→ 用同一槽位上次记下的那份。

只在同一槽位内复用，换槽位不会串味；`installed_props()` 本身不动，所以
`ui.rs::installed()`、`systems.rs`、`keys.rs` 的行为都不变。

## 验证

1. 进 Skyward → 从内部存储选官方包安装：
   - “正在安装”下面蓝色版本号，再下面**绿色一行**，是
     `/sdcard/Documents/…zip`；宽度不够时在最后一个 `/` 处折成两行，两行都完整。
   - 右侧“用时：mm:ss”照旧。
2. 装完（或任何一次翻页）：主页 ↔ 子页面来回滑动，分栏线上下**不再有灰闪圆角**。
3. 装完回到主页 → 关于 → “OS 版本 / Android 版本”两行仍然在。
4. `adb sideload` 装的包没有文件名，卡片不显示绿行（无回归）。

## 镜像

- `out/skyward-violin-OS4.0.5-20261119-2248.img`（104,857,600 B，ramdisk 49,798,379 B）
- SHA-256 `4efe1fd0d45cc62a90c96de430743459108eca6fe805fe0c6090c472f12338da`
- 内嵌 `system/bin/recovery` SHA-256 `62105f970c4df9ad4aa7106f641640fc5de0f52220d6ff7ddf0b792c6033de79`（与本机构建一致）
- ramdisk 1049 条目（含 `skyward/system-init`），`repack` 往返 RC=0
- clippy：`skyward-ui` 6 条、`skyward` 3 条，均为本轮之前既有；`skyward-install`、`skyward-info` 干净
- 未刷机、未写设备分区
