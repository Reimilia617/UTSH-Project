# UTSH 打包目录

| 路径 | 用途 |
| --- | --- |
| `etc/utsh/utsh.toml` | 系统级默认配置模板（deb 的 conffile / rpm 的 %config(noreplace)） |
| `man/utsh.1` | man page（打包时 gzip 到 man1） |
| `rpm/utsh.spec` | RPM spec |

产出的发行物（由 `scripts/package.sh` 生成到 `dist/`）：

- `utsh_<ver>_<arch>.deb` —— Debian/Ubuntu 包
- `utsh_<ver>-1.<arch>.rpm` —— Fedora/RHEL/openSUSE 包
- `utsh_<ver>_<arch>_fhs.tar.gz` —— FHS 布局全量包（rpm Source0，也是手动安装介质）
- `utsh_<ver>_linux_<arch>.tar.gz` —— 仅二进制（核心使用，无需 Node）
- `SHA256SUMS` —— 全部产物的校验和

版本号取自根 `Cargo.toml` 的 `[workspace.package] version`；架构取自
`dpkg --print-architecture`。

完整发布命令：

```bash
bash scripts/package.sh        # 产出 dist/ 下全部发行物
bash scripts/publish-release.sh  # 打 tag + 推送 GitHub Release（需 gh 或 GH_TOKEN）
```

系统安装/卸载（从 GitHub Release 拉取）：

```bash
bash scripts/install.sh        # root 执行
bash scripts/uninstall.sh      # root 执行
```
