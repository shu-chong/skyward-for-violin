# tools

`skyward-pack` 在生成 legacy 压缩的 ramdisk 时会调用外部 `lz4` 命令行工具
（`crates/skyward-bootimg/src/ramdisk.rs` 的 `lz4_tool()`）。没有它会在打包时报
`run lz4 (install it…)`。

本目录**不附带** `lz4.exe`：它是第三方二进制（本机原来用的是 lz4 v1.10.0 64-bit
multithread，Yann Collet），不应混进本仓库。请在目标机器上自行安装并放进 `PATH`：

```sh
# Windows (Chocolatey / Scoop)
choco install lz4
scoop install lz4

# Debian / Ubuntu
sudo apt install lz4

# macOS
brew install lz4
```

或者从 <https://github.com/lz4/lz4/releases> 取官方发行版，把可执行文件放到 `PATH`
里，或放到本目录后用 `PATH=$PWD/tools:$PATH` 启动 `skyward-pack`。

验证：`lz4 -V` 应能打印版本号。
