# C 方案落地：开机只跑 adb，解密后自动启用 mtp,adb

日期：2026-11-16 09:50
镜像：`out/skyward-violin-OS4.0.5-20261116-0950.img`
SHA-256：`8582f695ac713503273c4c910b7259dae3b16b0f44827aae5b8182c4ce7aec5f`
ramdisk：49,182,140 字节 / 104,857,600 字节；Android 二进制 8,836,640 字节
承接：`…2325.img`（`a022ee2b…d822`）

本轮**没有刷机、没有写入任何设备分区**。

## 你的要求

> C（推荐）开机仍只跑 adb；解密后才可以自动启用 mtp,adb
> 需要删除部分文字，设置最底下的：data分区解密完成后，每次亮屏都会先显示正平的 向右滑动……这一句话。

按这个改法，MTP **不需要手动按钮**：开机 adb-only，data 解密完成的那一刻自动切到 `mtp,adb`。
设置页「手势与安全」最底下那段说明整句删掉。

## 为什么不能只是把 `mtp` 改回 true

历史问题：开机 15.6 s `sys.usb.config=none → mtp,adb` 的 gadget 重绑会让 Windows 复合设备
重枚举并丢掉 ADB interface，20:36 那轮才把 `mtp = false`（不打包 `skyward-mtp.rc`、
`prop.default` 追加 `persist.sys.usb.config=adb`、运行时不启动 `Transfer`）。

但 `skyward-mtp.rc` 里 `on fs && configfs=1` 的动作只是**创建 `/dev/usb-ffs/mtp` 并 mount
functionfs**，不绑定 gadget、不改 `sys.usb.config`。没有它，`skyward_mtp::Service::start()`
打不开 functionfs，解密后也起不来 MTP。所以两者要解耦：

| | 开机 | 解密完成后 |
|---|---|---|
| `skyward-mtp.rc` | 在镜像里，创建 functionfs 节点 | 已就绪 |
| `sys.usb.config` | `adb`（`persist.sys.usb.config=adb`） | 自动切到 `mtp,adb` |
| `Transfer` 服务 | **不启动** | `decrypt_told()` 里启动 |

## 本轮改动

1. **`DeviceConfig.mtp_after_decrypt: bool`**（默认 false，`skyward-config`）：
   与 `mtp` 解耦的新开关。
2. **`devices/violin.toml`**：保持 `mtp = false`，新增 `mtp_after_decrypt = true`。
3. **`skyward-pack`**：打包条件由 `if cfg.mtp` 改为 `if cfg.mtp || cfg.mtp_after_decrypt`
   —— 这次镜像里**会有** `skyward-mtp.rc`；`prop.default` 的 `persist.sys.usb.config=adb`
   追加条件仍是 `if !cfg.mtp`，所以**开机依旧 adb-only**。`open()` 里
   `if cfg.mtp && app.state.file_transfer` 不动，开机不启动 `Transfer`、`begin()` 也不会
   `usb::switch(...)`，没有开机重绑。
4. **`skyward/src/ui.rs` 新增 `enable_mtp_after_decrypt()`**：
   - 条件：`mtp_after_decrypt` 开、`transfer` 还没起、`data.storage_open()`、
     且不在 `fastboot / sideloading / 擦除 / 安装` 中。
   - 动作：`Transfer::start()`（内部 `usb::set_mtp(true)`）→ `usb::switch(resting())`
     即 `mtp,adb` → 写 kmsg `file transfer came up once data was decrypted`。
     失败只写 kmsg，**不会让 recovery 挂掉**。
   - 之后主循环原有的 `offer_storage()` 会按 `data.storage_open()` 自动把 `/sdcard`
     提供给电脑；`withhold()` 在擦除/安装前照旧收回。
5. **调用点**：`decrypt_told()` 的 `match ended` 之后，
   `closed.is_empty() && data.storage_open()` 时触发 —— 覆盖「打开就已解密」和
   「输完密码全部解开」两条路径；解密未完成/失败不会触发。
6. **删文案**：设置页「手势与安全」底下的整段 footnote（关于锁屏右划的说明）删除，
   页面高度改用 `gesture.bottom`。双击亮屏开关及其副标题保留。

## 需要知道的一个代价

`usb::switch()` 必须经过 `none` 才能重新绑定 gadget，**adbd 会被 SIGKILL 再重启**，
Windows 有可能因此在解密完成这一刻丢掉 ADB interface。这正是 C 方案把它从
「开机 15.6 s」挪到「解密完成后」的原因 —— 时机可预期，且恢复办法是
`adb kill-server && adb devices`。MTP 盘符会在这次重绑后出现。
如果实测 Windows 丢 adb 后不影响你传文件，就这样用；若要彻底不掉 adb，
只能退回 B（只跑 adb + `adb pull/push`）。

## 构建与自检

| 项 | 结果 |
|---|---|
| 宿主 `cargo build --release -p skyward-pack` | RC=0（15.65 s） |
| Android `cargo build --release --target aarch64-linux-android -p skyward` | RC=0（1 m 22 s，无新增 warning） |
| 打包 | RC=0，ramdisk 49,182,140 B |
| **`skyward-mtp.rc`** | **在镜像里（1 条）** —— 与之前几版相反，这是本轮的关键差异 |
| `skyward/system-init` | 缺席（无双系统） |
| `prop.default` | `persist.sys.usb.config=adb`、`ro.adb.secure=0`（开机仍只跑 adb） |
| 镜像内 `device.toml` | `mtp=false`、`mtp_after_decrypt=true`、无振动、90Hz、rotate 270、columns 560 |
| 镜像内二进制 SHA | 与本地同为 `ec8e4186…f00` |
| 新 kmsg 标记 | 「file transfer came up once data was decrypted」「file transfer did not start after decryption」命中 |
| 删除的文案 | 「data 分区解密完成后，每次亮屏…」为 0 |
| ramdisk 往返 | `repack` RC=0，49,182,140 B 一致 |

## 刷入后请按顺序验证

1. **开机**：`sys.usb.config/state=adb`、`init.svc.adbd=running`、开机日志
   `xr_usb_composite_disconnect=0`，电脑**还没有** MTP 盘符；adb 全程不掉。
2. **解密完成**：日志出现 `file transfer came up once data was decrypted`，
   `sys.usb.state=mtp,adb`，电脑出现盘符并能看到 `/sdcard` 文件。
3. 若此刻 Windows 丢了 adb：`adb kill-server && adb devices` 恢复。
4. 设置页最底下不再有那句说明；解密页 3→2→1 真倒计时、锁屏「向右滑动解锁」、
   双击亮屏、WLAN 自动连接、本地更新无回归。
5. 侧载 / 擦除数据 / 安装时不自动切 MTP，`withhold()` 会先收回内部存储。

回退：`out/skyward-violin-OS4.0.5-20261115-2325.img`（`a022ee2b…d822`）。
只刷 `recovery_b`，`fastboot reboot recovery` 可用。
