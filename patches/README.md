# patches

`0001-violin-port.patch` 把**官方 skyward v0.3.1（commit `beef595`，即
`skyward-v0.3.1-source.zip` 解压后的 `kirklin-skyward-beef595/`）**变成上层目录这份
violin 适配源码。

- 63 个文件改动，其中 5 个新增，0 个删除，无二进制差异。
- 新增：`devices/violin.toml`、`crates/skyward-ui/src/tablet.rs`、
  `crates/skyward-sys/src/dvfs.rs`、`crates/skyward-vab/src/merge.rs`、
  `crates/skyward-install/src/image/protection.rs`。
- 覆盖 `Cargo.lock`、`Cargo.toml`、`.gitignore`、`crates/**`、`devices/**`。
- 不包含 `docs/`、`tools/lz4.exe`、构建产物与日志（它们不属于源码改动）。

## 在另一台电脑上应用

```sh
# 1) 取得官方 v0.3.1 源码，进入仓库根目录
cd kirklin-skyward-beef595

# 2) 如果手上是 zip 解压出来、没有 .git 的目录，先做一次基线提交
git init -q
git add -A
git -c user.name=base -c user.email=base@example.com commit -qm "official v0.3.1 beef595"

# 3) 检查并应用
git apply --check /path/to/0001-violin-port.patch
git apply            /path/to/0001-violin-port.patch
```

如果已经是一个位于官方 `beef595` 提交的 git 仓库，跳过第 2 步，直接 `git apply`。
不喜欢 git 也可以 `patch -p1 < 0001-violin-port.patch`，但 `git apply` 的错误信息更清楚。

## 已做过的校验

- 在官方源码副本上 `git apply --check` 通过，`git apply` 成功；
- 应用后 63 个文件逐一与本目录的源码做 SHA-256 比对，**63/63 完全一致**；
- patch 是文本格式，换行统一为 LF。

## 应用完之后

按上层 [`PORT.md`](../PORT.md) 的构建章节操作。需要另外准备：

1. Android NDK 29 + cargo-ndk（或按 `PORT.md` 手工导出 clang/ar 环境变量）；
2. 外部 `lz4` CLI，见 [`../tools/README.md`](../tools/README.md)；
3. 设备自己的 4.0.6 `recovery.img` 作为 `--stock` 底座——这是设备私有文件，不在仓库里。

如果只想直接构建而不关心改动过程，用上层目录的完整源码即可，不需要应用 patch。
