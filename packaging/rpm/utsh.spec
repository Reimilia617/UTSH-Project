# UTSH RPM spec（由 scripts/package.sh 渲染 @占位符@ 后交给 rpmbuild）。
#
# Source0 为 FHS 布局 tar 包（含 usr/bin、usr/share/utsh、etc/utsh 等），
# 由 scripts/package.sh 生成；直接对源码目录打包，不依赖互联网。

Name:           utsh
Version:        0.1.0
Release:        1%{?dist}
Summary:        UTSH - a Bash-compatible shell manager with a Zsh plugin ecosystem

License:        Apache-2.0
URL:            https://github.com/Reimilia617/UTSH-Project
Source0:        @FHS_TARBALL@
BuildArch:      @RPM_ARCH@
Requires:       git >= 2.0
Recommends:     nodejs >= 18

%description
UTSH（UT Shell）是一个兼容 Bash 语法、同时兼容 Zsh 插件与主题生态的交互式
Shell 及其管理工具。本包提供 utsh 命令行工具，可用于插件/主题/别名管理、
环境诊断（doctor）以及 WebUI 启动（需要 nodejs）。

%prep

%build

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}
tar -xzf %{SOURCE0} -C %{buildroot}

%files
%{_bindir}/utsh
%{_mandir}/man1/utsh.1.gz
%dir /etc/utsh
%config(noreplace) /etc/utsh/utsh.toml
%dir %{_datadir}/utsh
%{_datadir}/utsh/webui
%{_datadir}/utsh/utsh.example.toml
%{_docdir}/utsh

%post
# 注册为合法登录 shell：写入 /etc/shells
SHELL_PATH=%{_bindir}/utsh
if [ -f /etc/shells ]; then
    grep -qxF "$SHELL_PATH" /etc/shells || echo "$SHELL_PATH" >> /etc/shells
fi

%postun
if [ "$1" -eq 0 ]; then
    # 干净卸载：撤销 /etc/shells 注册，清理遗留空目录。
    sed -i "\\|^%{_bindir}/utsh\$|d" /etc/shells 2>/dev/null || true
    rmdir /etc/utsh 2>/dev/null || true
    rmdir %{_datadir}/utsh 2>/dev/null || true
fi

%changelog
* Fri Sep 05 2025 Reimilia617 <313009058+Reimilia617@users.noreply.github.com> - 0.1.0-1
- 首个发布：Rust 核心 + C++ FFI 解析器、CLI、WebUI 后端骨架
