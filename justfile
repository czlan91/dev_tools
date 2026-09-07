# Dev Tools 统一开发任务入口。
#
# Just 默认以找到的 justfile 所在目录执行任务，因此从项目子目录调用时，
# Cargo、打包脚本和输出路径仍然会正确指向项目根目录。

# 不指定任务时显示全部可用任务，而不是默认执行构建或发布操作。
default:
    @just --list

# 安装 Rust 开发组件并提前下载 Cargo 依赖。
# macOS 的 Xcode 和 Metal Toolchain 仍需按照 README 的系统级步骤安装。
setup:
    rustup component add rustfmt clippy
    cargo fetch

# 快速检查类型、借用关系和编译错误，不生成最终可执行文件。
check:
    cargo check

# 自动格式化全部 Rust 源码。
fmt:
    cargo fmt --all

# 只检查格式，不修改任何源码，适合 CI 或提交前检查。
fmt-check:
    cargo fmt --all -- --check

# 运行全部单元测试和集成测试。
test:
    cargo test

# 将 Clippy 警告视为错误，防止问题被警告信息掩盖。
lint:
    cargo clippy --all-targets -- -D warnings

# 依次执行提交前需要通过的质量检查。
verify: fmt-check check test lint

# 编译包含调试信息的开发版本，产物位于 target/debug/dev_tools。
build:
    cargo build

# 编译经过优化的发布版本，产物位于 target/release/dev_tools。
build-release:
    cargo build --release

# 使用默认 info 日志级别编译并运行应用。
run:
    cargo run

# 使用 debug 日志级别运行应用，适合直接观察详细诊断日志。
run-debug:
    RUST_LOG=debug cargo run

# 构建开发版本后进入 LLDB；进入调试器后执行 run 启动应用。
debug: build
    lldb target/debug/dev_tools

# 生成带自定义 Dock 图标和本地 ad-hoc 签名的 macOS 应用包。
package:
    echo "[package] 编译 Dev Tools 的 release 版本"
    cargo build --release
    echo "[package] 创建 macOS 应用包目录"
    install -d target/release/bundle/DevTools.app/Contents/MacOS target/release/bundle/DevTools.app/Contents/Resources
    echo "[package] 复制可执行文件、Info.plist 和 Dock 图标"
    install -m 755 target/release/dev_tools target/release/bundle/DevTools.app/Contents/MacOS/dev_tools
    install -m 644 packaging/macos/Info.plist target/release/bundle/DevTools.app/Contents/Info.plist
    install -m 644 assets/icons/AppIcon.icns target/release/bundle/DevTools.app/Contents/Resources/AppIcon.icns
    echo "[package] 为本地应用包添加 ad-hoc 签名"
    codesign --force --deep --sign - target/release/bundle/DevTools.app
    echo "[package] 完成：target/release/bundle/DevTools.app"

# 打包后通过 macOS Launch Services 启动 .app，以验证 Dock 图标和应用包。
open: package
    open target/release/bundle/DevTools.app

# 生成本地发布产物，但不上传到任何外部平台。
# 输出包括 ZIP 应用包和对应的 SHA-256 校验文件。
publish: verify package
    mkdir -p dist
    rm -f dist/DevTools-macos.zip dist/DevTools-macos.zip.sha256
    ditto -c -k --sequesterRsrc --keepParent target/release/bundle/DevTools.app dist/DevTools-macos.zip
    shasum -a 256 dist/DevTools-macos.zip > dist/DevTools-macos.zip.sha256
    @echo "[publish] 发布文件：dist/DevTools-macos.zip"
    @echo "[publish] 校验文件：dist/DevTools-macos.zip.sha256"
