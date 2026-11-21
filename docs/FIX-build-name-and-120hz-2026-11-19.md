# 构建产物带版本号；120Hz 试验（2026-11-19）

## 1. 镜像命名带主版本号

`skyward-pack recovery` 的 `--out` 改为可选。不给时按

```
out/skyward-<CARGO_PKG_VERSION>-<codename>-<stock build>-<YYYYMMDD-HHMM>.img
```

命名，例如 `skyward-0.3.1-violin-OS4.0.6-20261119-2234.img`：

| 段 | 来源 |
|---|---|
| `0.3.1` | `CARGO_PKG_VERSION`，即 workspace `Cargo.toml` 的版本 |
| `violin` | `devices/violin.toml` 的 `[device] codename` |
| `OS4.0.6` | **底座 ramdisk 里 `prop.default` 的 `ro.build.version.incremental`**（`OS4.0.6.0.XOTCN` 取前三段），不是配置里写的，换底座自动跟着变 |
| `20261119-2234` | 构建时刻，本地时区 |

`write()` 现在会补建缺失的父目录（`out/` 不存在也能写）。

时区踩坑：只开 `tzdb-bundle-always` 时 `jiff::TimeZone::system()` 在 Windows 上回退到 UTC，
文件名出现 `1430` 而不是 `22:30`。加 `tz-system` feature 后正确。

## 2. 120Hz 试验

历史（`docs/ALL-FIXES-MASTER-2026-11-17.md`）：追 144Hz 时 `dpu_underflow` 903 次、花屏，
日志里 `last vote: ddr 0 MB/s, clk profile 1` —— 显示控制器**没有向 DVFS 投票**，DDR 停在最低档。
90Hz 实测 0 次 underflow。120Hz 从未试过。

面板 modes：`3200x2136x{120,144,90,60,50,48,30}`。

### 做法

1. `devices/violin.toml`：`refresh = 90` → `120`。
2. 新增 `crates/skyward-sys/src/dvfs.rs::hold_memory_fast()`：读
   `/sys/class/devfreq/ddr_devfreq/available_frequencies` 取最大值写进 `min_freq`，
   把内存控制器钉在最高频。
3. `crates/skyward/src/ui.rs`：`KmsSurface::open` 之前，`refresh > 90` 时调用；
   失败只写 kmsg（`display: the memory controller keeps its own rate (…)`），不影响启动。

### 为什么可能有效、也可能无效

- **可能有效**：如果 underflow 的瓶颈是 DDR 带宽（DPU 读 framebuffer 供不上），钉住最高频就能喂饱。
- **可能无效**：如果瓶颈是 DPU 自身的像素时钟（`clk profile 1` 这一项），用户态无法提频，
  那 120Hz 仍会 underflow，只能改内核 DTS/驱动或开 DSC。
- 144Hz 暂不试：像素时钟又高 20%，即使 120 成功也不一定够；且历史已证 144 必花屏。

### 真机验证

```
adb shell "dmesg | grep -i 'new refresh rate'"     # 期望 120
adb shell "dmesg | grep -ci underflow"             # 期望 0
adb shell "cat /sys/class/devfreq/ddr_devfreq/min_freq"  # 期望最高频
adb shell "dmesg | grep -i 'keeps its own rate'"   # 有则说明 min_freq 没写进去
```

花屏、闪烁、局部错位 = underflow，直接回刷 90Hz 那一版。

## 3. 镜像

| 文件 | 用途 | SHA-256 |
|---|---|---|
| `out/skyward-0.3.1-violin-OS4.0.6-20261119-2231.img` | **90Hz 稳定**（回退用） | `f4842f899ce524334b852ef653dd780d58827d33a163cd809a85a9e34cbe091b` |
| `out/skyward-0.3.1-violin-OS4.0.6-20261119-2234.img` | **120Hz 试验** | `965d335b7f4d033a00ccd35be457bdcfff64f95718eb515c1e9293e44d797052` |

- 两版都是 4.0.6 底座、`skyward` 0.3.1、本轮 UI 排版（路径上移 / 版本号 25.3dp / 间距拉开）。
- 2231 与之前的 `skyward-violin-OS4.0.6-20261119-2220.img` **内容完全相同**（SHA 一致），只是文件名换成了新规则。
- 2234：ramdisk 49,800,306 B，1051 条，内嵌 recovery `b8f3340c…a3d492` 与本机构建一致，`repack` RC=0。
- 2231：ramdisk 49,798,439 B，内嵌 recovery `3eb13f78…5f47e1`。

## 4. 已知限制

- 120Hz 的实际效果只能真机验证，本轮未刷机。
- DDR 拉频会提高待机功耗；只在 `refresh > 90` 时执行，回 90 即无影响。
