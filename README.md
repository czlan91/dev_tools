# Dev Tools

一个使用 Rust、GPUI 和 gpui-component 开发的桌面工具箱。目前包含 TSV 转 SQL `IN` 条件和图片转 Base64 等功能。

项目入口、模块职责和新增工具的完整步骤见 [架构与工具开发指南](ARCHITECTURE.md)。

## 环境准备

本项目当前面向 macOS 开发，使用 Rust Edition 2024，最低支持 Rust 1.90。开始前需要安装 Rust 工具链和 Apple Command Line Tools。

```bash
# 安装 Apple Command Line Tools（已安装时无需重复执行）
xcode-select --install

# 安装 Rust 工具链；安装完成后按 rustup 的提示加载环境变量
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 确认 Rust 和 Cargo 可以正常使用
rustc --version
cargo --version
```

如果本机 Rust 低于 1.90，请升级稳定版工具链：

```bash
rustup update stable
```

以上命令需要在终端中运行。项目相关的 Cargo 命令均应在仓库根目录执行。

### Just 任务运行器

项目使用 `justfile` 统一管理编译、测试、运行、打包和发布任务。macOS 可以通过 Homebrew 安装 Just：

```bash
brew install just
just --version
```

安装完成后执行环境准备任务，它会安装 Rustfmt、Clippy 并下载 Cargo 依赖：

```bash
just setup
```

### Metal Toolchain（gpui 必需）

gpui 在 macOS 上构建时需要编译 Metal shader。如果 `cargo check` / `cargo build` 报
`missing Metal Toolchain` 错误，执行以下命令安装（需要先完整安装 Xcode，仅 Command Line Tools 不够）：

```bash
xcodebuild -runFirstLaunch
xcodebuild -downloadComponent MetalToolchain
```

安装完成后重新构建即可。

## 项目依赖

Rust 依赖统一声明在 `Cargo.toml` 中：

- `gpui`：桌面窗口、渲染和应用运行时。
- `gpui-component`：侧边栏、输入框、按钮等界面组件。
- `base64`：图片 Base64 编码。
- `anyhow`：仅在 `main.rs` 的 `AssetSource` trait 实现中使用（该 trait 由 gpui 定义，签名要求 `anyhow::Result`）。
- `thiserror`：项目统一的错误类型定义（见 `src/error.rs`），为每个失败场景提供语义化错误枚举。
- `log`：日志门面接口（业务代码只依赖它）。
- `env_logger`：日志输出实现（写入 stderr，由 `RUST_LOG` 控制级别）。
- `serde` + `serde_json`：设置文件 JSON 序列化/反序列化，以及主题文件解析。
- `json5`：JSON5 格式解析（JSON 超集，支持注释、尾逗号、单引号）。

无需逐个手动安装 Rust 包。进入项目根目录后执行以下命令，Cargo 会根据 `Cargo.toml` 和 `Cargo.lock` 下载并锁定全部依赖：

```bash
cargo fetch
```

## 编译与检查

推荐使用 Just：

```bash
# 查看全部任务
just

# 编译开发版本
just build

# 编译发布版本
just build-release

# 执行格式、编译、测试和 Clippy 检查
just verify
```

对应的原始 Cargo 命令如下：

```bash
# 快速检查类型和编译错误，不生成最终可执行文件
cargo check

# 开发模式编译；产物位于 target/debug/dev_tools
cargo build

# 发布模式编译；产物位于 target/release/dev_tools
cargo build --release
```

## 运行与启动

开发时可以让 Cargo 编译并立即启动应用：

```bash
just run

# 等价的 Cargo 命令
cargo run
```

也可以先编译，再直接启动生成的程序：

```bash
# 启动开发构建
./target/debug/dev_tools

# 启动发布构建
./target/release/dev_tools
```

## 日志与问题排查

应用通过 `log` + `env_logger` 输出日志到 **stderr**（终端）。日志级别由环境变量 `RUST_LOG` 控制，未设置时默认 `info`（关键操作与错误），`debug` 会额外输出更细的过程信息。

```bash
# 默认级别（info）：窗口创建、工具切换、转换/读取结果、错误
./target/debug/dev_tools

# 打开全部 debug 日志
RUST_LOG=debug ./target/debug/dev_tools

# 只看「图片工具」相关日志（按 target 过滤）
RUST_LOG=tool.image=debug ./target/debug/dev_tools
```

日志要点：

- 记录的是**关键操作、失败位置和耗时任务**（如选中/读取图片、TSV 转换、复制），用于还原「发生了什么」。
- **不会**记录完整输入内容、图片 Base64、密码等敏感数据。
- 用 `cargo run` 启动时日志同样打印在运行 `cargo run` 的终端里，排查后按 `Ctrl+C` 结束即可。
- 日志 target 按功能模块命名：`tool.tsv`、`tool.json`、`tool.image`、`settings`、`theme`、`assets`。

更多日志设计约定见 [架构与工具开发指南](ARCHITECTURE.md) 的「错误处理与日志」一节。

## 项目结构

```
src/
├── main.rs                  # 应用入口、资源加载器、窗口初始化
├── app.rs                   # 应用壳层：侧边栏、工具切换、状态栏
├── error.rs                 # 统一错误类型（thiserror）
├── settings/
│   ├── mod.rs               # 设置状态管理、持久化（JSON 文件）
│   ├── panel.rs             # 设置面板 UI
│   └── theme.rs             # 主题加载和应用
└── tools/
    ├── mod.rs               # 工具模块声明和 ToolId 枚举
    ├── tsv_to_sql.rs        # TSV → SQL IN 工具
    ├── image_to_base64.rs   # 图片 → Base64 工具
    ├── json_formatter.rs    # JSON/JSON5 格式化工具
    ├── json_compare.rs      # JSON 比较工具
    ├── json_diff.rs         # JSON 比较引擎（私有模块）
    └── json_utils.rs         # JSON 公共工具函数（私有模块）
```

## 打包

项目提供 macOS 应用包脚本。它会执行 release 编译、组装 `DevTools.app`、复制 Dock 图标，并为本地应用添加 ad-hoc 签名：

```bash
just package
```

生成的应用位于 `target/release/bundle/DevTools.app`。通过应用包启动时，Dock 会显示项目的自定义图标：

```bash
open target/release/bundle/DevTools.app
```

直接执行 `cargo run` 启动的是未放入 `.app` 的裸二进制，macOS 不会读取应用包中的 `Info.plist` 和 `AppIcon.icns`，因此开发运行时仍可能显示系统默认图标。

如只需要发布模式的裸可执行文件，可以继续运行：

```bash
cargo build --release
```

该文件位于 `target/release/dev_tools`。正式对外分发应用时，还需要使用 Apple Developer 证书签名并完成公证。

## 本地发布产物

`publish` 会先执行全部质量检查和 macOS 打包，再生成 ZIP 与 SHA-256 校验文件。该任务只生成本地文件，不会上传到 GitHub 或其他平台：

```bash
just publish
```

输出文件：

- `dist/DevTools-macos.zip`
- `dist/DevTools-macos.zip.sha256`

## 测试与代码质量

```bash
# 编译并运行全部测试
cargo test

# 检查常见代码问题；clippy 组件通常随默认 rustup 安装提供
rustup component add clippy
cargo clippy --all-targets

# 检查代码格式
cargo fmt --all -- --check

# 自动格式化代码
cargo fmt --all
```

## Debug

使用 Just 构建并进入 LLDB：

```bash
just debug
```

如果只需要开启详细日志运行应用：

```bash
just run-debug
```

先生成包含调试信息的开发构建：

```bash
cargo build
```

使用 macOS 自带的 LLDB 启动调试器：

```bash
lldb ./target/debug/dev_tools
```

进入 LLDB 后，可以设置断点并启动程序：

```text
breakpoint set --name main
run
```

也可以在 RustRover 中打开项目，在需要的位置添加断点，然后运行或调试 Cargo 的 `run` 配置。
