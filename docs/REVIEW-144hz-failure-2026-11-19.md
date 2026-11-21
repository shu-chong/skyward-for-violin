# 144 Hz 真机失败复盘（2026-11-19 23:16）

镜像：`out/skyward-0.3.1-violin-OS4.0.6-144hz-test-20261119-2314.img`
（SHA `a7ec4961…20ac2fc`，与 120 Hz 版只差 `skyward/device.toml` 里的 `refresh`）
现象：进 Skyward 即花屏。
原始日志：`logs/20261119-2316/{dmesg-144.txt,dmesg-144b.txt}`。

**结论：144 Hz 在 recovery 里做不到，卡的是 DPU 核心时钟，不是 DDR。**
把 DDR 钉到最高频这一招对 120 Hz 有效，对 144 Hz 无效。

---

## 1. 面板确实是 144 Hz，DDR 也确实钉住了

```
mode: "3200x2136x144": 144 1035492 3200 3225 3237 3245 2136 2162 2164 2216
      → 1,035,492,000 / (3245 × 2216) = 143.9999 Hz

/sys/class/devfreq/ddr_devfreq/min_freq = 8533000000
/sys/class/devfreq/ddr_devfreq/cur_freq = 8533000000
```

`hold_memory_fast()` 这次同样成功（没有 `display: the memory controller keeps its own rate`）。
所以"花屏是因为忘了拉 DDR"可以排除。

## 2. 花屏就是 underflow，而且是持续的

```
[   14.995258] [DPU-W]: [CORE]: dpu_underflow_work: pipe 0, DSI-1(frame_no 0): underflow,
              vrefresh_rate 144(hz), cur 11073411us, underflow 11069823us
[   14.995278] [DPU-I]: [DFX]: do_dpu_core_perf_dump: last vote: ddr 0 MB/s, clk profile 1
[   14.995288] [DPU-I]: [DFX]: do_dpu_core_perf_dump: real time: ddr 40958 MB/s, real core clk 483333333, axi clk 557056000
```

- 从 **14.99 s**（第一次 commit）就开始，一直不停。
- 空闲、没人碰屏幕时也一样：56 秒窗口内 144 次 `dpu_underflow_work`，**2.57 次/秒**持续。
- 到 42 s 时 `err_count` 已累到 91，之后被驱动限流（`UNDERFLOW suppressed`）。

空转都下溢 → 不是"某次刷新的突发写回"，是**扫描输出这个动作本身供不上**。

## 3. 真正的瓶颈：DPU 核心只有 483.33 MHz

| | 120 Hz（好） | 144 Hz（花） |
| --- | --- | --- |
| 像素钟 | 912,819 kHz | 1,035,492 kHz |
| 扫描率（含消隐） | 0.913 Gpx/s | 1.035 Gpx/s |
| DPU 核心时钟 | 483,333,333 Hz | 483,333,333 Hz（同一档） |
| 核心每时钟要处理的像素 | **1.89** | **2.14** |
| 结果 | 0 underflow | 2.57 次/秒 |

`clk_dpucore0_dvfs = 483333333`，`clk_ppll2_dpu = 1450000000`（PLL 是 1.45 GHz，核心是 ÷3）。
DPU 每时钟大约吞吐 2 像素，上限 ≈ 966 Mpx/s；120 Hz 要 913 Mpx/s（94.4%，勉强够），
144 Hz 要 1035 Mpx/s（107%，必超）。这正好卡在 120 稳、144 崩的分界上。

DPU 自己的 dump 也说明了原因：`last vote: ddr 0 MB/s, clk profile 1` ——
recovery 里的 DRM 提交不带任何性能投票，驱动就用最低可用档 `profile 1`。Rust 侧的
`hold_memory_fast()` 只能改 DDR 的 devfreq，改不到 DPU 的档位。

## 4. 顺便否掉的两个假设

- **不是 DDR 带宽**：驱动自己测到 `real time: ddr 40958 MB/s`（40.9 GB/s），
  144 Hz 实际只需要约 4 GB/s 读 + 4 GB/s 写。差了十倍。
- **不是 PLL 不够**：`clk_ppll2_dpu` 跑在 1.45 GHz，÷2 就是 725 MHz，
  足够 144 Hz（只需约 518 MHz @2px/clk）。是**分频档位没被驱动选上去**，不是 PLL 上限。

## 5. 试过从用户态直接改，改不动

`/sys/kernel/debug/clk/clk_dpucore0_dvfs/clk_rate` 是可写的（`-rw-r--r--`），试了一次：

```
echo 600000000 > /sys/kernel/debug/clk/clk_dpucore0_dvfs/clk_rate
→ readback 26112000          # 被钳到 26.1 MHz，根本不是 600
echo 483333333 > …/clk_rate  # 立即写回
→ readback 483333333
```

说明这个 DVFS 时钟只认自己表里的档位，表外值会被钳到最低档；而且 DPU 驱动每次 commit 都会
按自己的 perf 计算重设，用户态写的值留不住。**已恢复 483.33 MHz，系统未崩溃、uptime 连续。**

## 6. 处置

- **回刷 120 Hz**：`out/skyward-0.3.1-violin-OS4.0.6-120hz-test-20261119-2234.img`
  （实测 0 underflow，含滑动翻页与息屏唤醒）。
- `devices/violin.toml` 已改回 `refresh = 120`，注释里记下 144 的结论，避免下次误构。
- 144 Hz 镜像 `…-144hz-test-20261119-2314.img` 保留作为反例，不要再用。

## 7. 真想要 144 的话

得动内核/驱动，不在 recovery 用户态能到的范围：

1. 让 `dpu_core_perf` 的 `clk profile` 走到 ≥ 2.2 px/clk 的档（PPLL 1.45 GHz ÷2 = 725 MHz）；
2. 或者开 DSC（显示流压缩），把像素率压到 2 px/clk 以下；
3. 两条都得有厂商的 perf 投票逻辑，而 recovery 里没有 composer 来投。

120 Hz 已经吃掉 94.4% 的 DPU 能力，余量只剩 5.6%——这是这台机器在 recovery 里的天花板。
