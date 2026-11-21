# 安装卡片：路径上移、版本号再放大、上下留白拉开（2026-11-19）

## 用户要求

- `17OS4.0.261112.190429467.XRPDCN.S` **再大一号**；
- 版本号与**上面**（包路径）和**下面**（`第1步，共3步：写入新系统：几 %`）的间距**拉开**；
- 目标版面：

```
正在安装                          用时：05:12
/sdcard/Documents/violin-ota_full-OS4.0.6.0.XOTCNXM-user-17.0-e0b0fc6c62.zip
17OS4.0.261112.190429467.XRPDCN.S
第1步，共3步：写入新系统：几 %
```

## 改动

`crates/skyward-ui/src/widgets.rs::progress_card_named`

### 1. 顺序：路径移到版本号上面

原来是「标题 → 版本号（蓝）→ 路径（绿）→ 阶段行」；按用户示意改为
「标题 → **路径（绿）** → **版本号（蓝）** → 阶段行」。

sideload（没有路径）时版本号直接接在标题下，间距同 18dp。

### 2. 版本号再大一号

| | 之前 | 现在 |
|---|---|---|
| 字号 | `row_title_px + dp(4.2)` = **21.1dp** | `row_title_px + dp(8.4)` = **25.3dp** |
| 行高 | `px × 1.18` | 不变（29.9dp） |
| 颜色 | `pal.accent` | 不变 |

25.3dp 是页面小标题（16.9dp）的 1.5 倍。28 字符的版本号在 violin 的文字区
（约 1000dp）里仍是一行；窄屏会按字断行，不会出省略号。

### 3. 间距：改成按字形实际墨迹定位

原来各块用「上一行基线 + 固定 dp − 新字体 ink_top」推算，间距会随字号浮动，
版本号一放大就把上下顶得很紧。现在每块都按 `Face::ink(px)` 的实际墨迹顶/底
排布，参数就是**看得见的空隙**：

| 间隙 | 值 |
|---|---|
| 标题 → 路径 | 14dp |
| 路径末行 → 版本号 | **18dp** |
| 版本号 → 阶段行 | **16dp** |
| 路径行距 | `sub_px × 1.36`（19.0dp，原 1.3） |
| 阶段行 → 进度条 | 13dp（不变） |

卡片高度因此在 violin 上从约 145dp 长到约 189dp（版本号一行 + 路径一行时），
页面其余内容（footnote、确认按钮）`bottom` 返回值自动跟随下移。

## 镜像

- `out/skyward-violin-OS4.0.5-20261119-2218.img`（104,857,600 B，ramdisk 49,798,433 B）
- SHA-256 `29a3fd25cab8309498a47946589784212a0fc832c1edb63da2513c83702dd7ff`
- 内嵌 `system/bin/recovery` = `3eb13f78b126483f088e84d7cfd82c01e95d125f560c38291f486561be5f47e1`，与本机构建一致
- ramdisk 1051 条（与 2248 版逐条一致，只有 recovery 二进制与长度变化）
- `repack` 往返 RC=0，ramdisk 49,798,433 B 不变
- clippy：`skyward-ui` 6 条，全部是本轮之前就有的（含 `progress_card_named` 参数 10/7 那条）；无新增

## 验证

1. 装官方包时，卡片自上而下应为：`正在安装`（右端计时器）→ 绿色路径 → 蓝色大版本号 → `第1步，共3步…` → 进度条。
2. 路径一行放得下就一行（violin 上是这样）；放不下才会断成 `文件夹/` + 文件名。
3. 版本号上下应明显比路径行、阶段行之间松。

## 未做

- footnote 与卡片之间、页面标题与卡片之间未动（用户指的主要是卡片内部）。
- `skyward-preview` 在 Windows 上仍编不出来（依赖 `skyward-info` 的 unix 代码），本轮同样无法先出图。

## 后续：底座换成官方 4.0.6（同一份代码，镜像 2220）

用户给了 4.0.6 的 DNA 解包目录 `D:/DNA_WIN-230828/DNA_violin-ota_full-OS4.0.6.0.XOTCNXM-user-17.0`，
其中 `001_DNA/recovery.img` 就是官方 4.0.6 的 recovery 分区。

### 两版 recovery 的实际差异

| 比较项 | 结果 |
|---|---|
| 镜像 SHA-256 | 4.0.5 `376631cf…0293` / 4.0.6 `52dd68c2…3d1d`（**不同**） |
| 头部（`ANDROID!` + 各字段） | 除 ramdisk 长度 4 字节外**逐字节相同** |
| ramdisk 条目 | 各 **1037 条**，路径与每个文件大小**完全一致** |
| `system/bin/init`、`system/bin/recovery`、`plat_file_contexts`、`init.recovery.O1_asic.rc` | **哈希相同** |
| `prop.default` | **不同**：只有版本身份字符串 |

`prop.default` 的差异全部是身份，不是功能：

```
-ro.build.version.incremental=OS4.0.5.0.XOTCN
+ro.build.version.incremental=OS4.0.6.0.XOTCN
-ro.system.build.fingerprint=…/OS4.0.5.0.XOTCN:user/release-keys
+ro.system.build.fingerprint=…/OS4.0.6.0.XOTCN:user/release-keys
（另含 ro.build.date / date.utc / host / description / ro.system.build.date）
```

也就是说 4.0.6 没有改 recovery 的任何代码、策略或库，只把"我是哪个版本"改了。
压缩后 29,254,685 → 29,254,691（+6 字节），正是这些字符串影响压缩率的结果。

### 内核

`001_DNA/boot.img` 里的内核是 `Linux version 6.18.21-android17-5-g284297031c53-abogki553682993-4k`，
与设备在 Skyward 里 `uname -r` **完全一致**——recovery 分区不带内核，内核来自 boot 分区，
已经跟着 OTA 走到 4.0.6 了，不存在"旧内核加载新模块"的问题。

### 换底座之后

- 新底座 `.references/stock406-recovery.img`（SHA-256 `52dd68c23b9d3ba7b08ce789cff8bd99bfc8b7dbe70bcfea33ea77d9ddd23d1d`）
- 镜像 `out/skyward-violin-OS4.0.6-20261119-2220.img`
  - SHA-256 `f4842f899ce524334b852ef653dd780d58827d33a163cd809a85a9e34cbe091b`
  - ramdisk 49,798,439 B（比 4.0.5 底座多 6 字节）
  - 内嵌 `system/bin/recovery` = `3eb13f78…5f47e1`，与本机构建一致
  - 与 2218（4.0.5 底座）逐条对比：路径与大小 **0 差异**
  - `repack` 往返 RC=0
- 成品镜像里 `prop.default` 已是 `ro.build.version.incremental=OS4.0.6.0.XOTCN`，
  即 Skyward 起来后 `getprop` 会报 4.0.6，不再被误认成 4.0.5。
- 功能行为与 2218 **完全相同**（代码只差 UI 排版那一处，底座只差版本字符串）。
