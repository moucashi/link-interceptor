# Link Interceptor

一个使用 Rust 和 `floem` 编写的 Windows 桌面 URL 拦截器。

此应用可以注册为 `HTTP` 和 `HTTPS` 的默认浏览器候选项。当其他程序通过系统浏览器打开 URL 时，Windows 可以将该 URL 作为第一个命令行参数传递给此应用启动。随后，应用会允许你编辑、复制、收藏、保存该 URL，并将其转交给已安装的浏览器或其他已注册/自定义的处理程序打开。

## 功能

- 拦截作为第一个 CLI 参数传入的 URL/deeplink。
- 编辑和复制被拦截的 URL。
- 自动将被拦截的 URL 保存到 YAML 历史记录。
- 添加/移除收藏。
- 从 Windows registry 中发现已安装的浏览器。
- 发现 URL protocol handler，例如 `mailto:` 或自定义 scheme。
- 配置自定义应用和域名规则。
- 在无需管理员权限的情况下，将当前可执行文件注册/反注册到 HKCU。
- 打开 Windows Default Apps 设置，让用户选择此应用。
- 保持单个后台进程：后续启动会聚焦主窗口，或请求正在运行的进程打开新的拦截窗口。

## 构建

```powershell
cargo build --release
```

无参数运行时打开主界面。如果应用已经在运行，本次启动会在请求正在运行的进程显示并聚焦主窗口后退出：

```powershell
cargo run
```

携带 URL 运行时只打开拦截窗口。如果应用已经在运行，本次启动会在请求正在运行的进程创建新的拦截窗口后退出：

```powershell
cargo run -- "https://example.com"
cargo run -- "mailto:test@example.com"
```

## 数据文件

应用会将 YAML 文件存储在：

```text
%APPDATA%\LinkInterceptor\config.yaml
%APPDATA%\LinkInterceptor\history.yaml
%APPDATA%\LinkInterceptor\favorites.yaml
```

## Windows 注册

打开“注册”标签页并点击“注册当前 exe”。该操作只会为当前可执行文件路径写入 HKCU key，并将应用注册为浏览器候选项。Windows 10/11 会保护实际的默认应用选择，因此本应用不会写入 `UserChoice`。点击“打开默认应用设置”，然后在 Windows 设置中选择 Link Interceptor。

如果移动了便携版可执行文件，请在新路径下重新注册。

## 当前范围

这是 v1 便携版可执行文件实现，目前有意不包含安装器或自动更新。单实例 IPC 使用本地 loopback listener 实现；每个进程只保留一个主窗口，但可以打开多个拦截窗口。
