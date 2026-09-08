# Dev Tools 架构与工具开发指南

本文介绍当前项目的代码入口、模块职责、界面运行流程，以及如何添加一个新的工具。文档以当前代码为准，适合在阅读源码或继续开发时配合使用。

## 1. 技术栈

- Rust Edition 2024：主要开发语言，项目声明的最低工具链版本为 Rust 1.90。
- GPUI：窗口、应用上下文、状态实体和界面渲染。
- gpui-component：侧边栏、按钮、输入框等常用界面组件。
- Cargo：依赖管理、编译、测试和打包。

依赖版本和完整开发命令见 `Cargo.toml` 与 `README.md`。

## 2. 当前目录结构

```text
dev_tools/
├── Cargo.toml                 # 项目信息、Rust 版本和依赖声明
├── justfile                   # 编译、检查、运行、打包和发布任务入口
├── README.md                  # 环境、依赖、编译、运行、打包和调试命令
├── REQUIREMENTS.md            # 功能需求记录
├── ARCHITECTURE.md            # 当前架构和新增工具开发指南
├── assets/
│   ├── themes/
│   │   ├── dark.json       # 深色主题（编译期内嵌）
│   │   └── light.json      # 浅色主题（编译期内嵌）
│   └── icons/
│       ├── app-icon-1024.png  # Dock 图标的 1024px PNG 源文件
│       └── AppIcon.icns       # macOS 应用包使用的多尺寸图标
├── packaging/macos/
│   └── Info.plist             # macOS 应用元数据和图标声明
└── src/
    ├── main.rs                # 程序入口：初始化 GPUI 并创建主窗口
    ├── error.rs               # 应用级错误类型（thiserror 派生）
    ├── app.rs                 # 应用外壳：持有工具状态、设置、菜单路由
    ├── settings.rs            # 设置持久化：菜单位置、主题选择（JSON 落盘）
    ├── theme.rs               # 主题管理：浅色/深色/跟随系统（编译期内嵌主题文件）
    └── tools/
        ├── mod.rs             # 工具模块声明和 ToolId 定义
        ├── tsv_to_sql.rs      # TSV 转 SQL IN 工具
        ├── image_to_base64.rs # 图片转 Base64 工具
        └── json_formatter.rs  # JSON 格式化 / JSON5 格式化 / key 排序 / diff 工具
```

项目遵循“一个功能一个文件或目录”的原则。简单工具放在 `src/tools/<工具名>.rs`；包含多个紧密关联模块的复杂工具应使用 `src/tools/<工具名>/` 目录，并通过其中的 `mod.rs` 暴露对外类型。

## 3. 程序入口与启动流程

程序入口是 `src/main.rs` 中的 `main` 函数。启动流程如下：

```text
main()
  │
  ├─ 创建 GPUI Application
  ├─ 注册 FsAssetSource，用于从文件系统读取图片等资源
  └─ application.run(...)
       │
       ├─ 初始化 gpui-component
       ├─ 配置窗口尺寸、最小尺寸和标题栏
       └─ 打开主窗口
            │
            ├─ 创建 DevToolsApp
            └─ 使用 gpui_component::Root 包裹应用根组件
```

各步骤的职责：

1. `Application::new()` 创建 GPUI 应用运行时。
2. `with_assets(FsAssetSource)` 注册资源加载器。界面的 `img(...)` 等 API 可以借此读取文件。
3. `gpui_component::init(cx)` 初始化组件库，必须在使用 gpui-component 控件之前调用。
4. `WindowOptions` 设置主窗口的默认大小、最小大小和标题栏。
5. `cx.open_window(...)` 创建窗口，并在窗口中创建 `DevToolsApp`。
6. `gpui_component::Root` 是窗口最外层组件，为内部组件提供主题等基础能力。

`main.rs` 应只负责应用启动和全局初始化，不应放入 TSV、JSON 或图片处理等具体业务逻辑。

## 4. 应用外壳与菜单路由

`src/app.rs` 中的 `DevToolsApp` 是所有工具的上层容器，目前保存：

- `active: ToolId`：当前选中的工具。
- `settings: Entity<Settings>`：应用设置实体（菜单位置、主题选择）。
- `tsv: Entity<TsvTool>`：TSV 工具的状态实体。
- `image: Entity<ImageTool>`：图片工具的状态实体。
- `json: Entity<JsonFormatterTool>`：JSON 格式化工具的状态实体。

GPUI 的 `Entity<T>` 可以理解为由框架管理的、可更新并能触发重新渲染的状态对象。工具切换过程如下：

1. 用户点击 `SidebarMenuItem`。
2. 点击回调通过 `root.update(...)` 修改 `DevToolsApp.active`。
3. 调用 `cx.notify()` 通知 GPUI 状态已经变化。
4. GPUI 再次调用 `DevToolsApp::render`。
5. `match self.active` 选择应显示的工具实体。

### 4.1 设置与持久化

设置（`src/settings.rs`）保存在 `~/.config/dev_tools/settings.json`，每次变化立即写盘，避免数据丢失。当前支持两类设置：

- **菜单位置**（`MenuPosition`）：`Left` 或 `Right`，控制侧边栏在窗口的哪一侧。`render` 中根据该值传入不同的 `Side` 枚举，并调整顶层 `h_flex` 中子元素的排列顺序。
- **主题选择**（`ThemeChoice`）：`Light`、`Dark` 或 `System`，由 `src/theme.rs` 的 `apply` 函数实际应用。

### 4.2 主题管理

`src/theme.rs` 在编译期通过 `include_str!` 将 `assets/themes/*.json` 内嵌到二进制中，启动时即生效，打包后不依赖外部文件。主题应用流程：

1. `DevToolsApp::new` 中读取保存的主题选择，调用 `theme::apply` 设置初始主题。
2. 用户点击设置菜单项，更新 `Settings` 实体（落盘 + notify），再调用 `theme::apply` 刷新窗口。
3. 跟随系统模式：`render` 每次调用时对比当前 `ThemeMode` 与系统外观，不一致时重新应用，不会死循环。

### 4.3 侧边栏菜单

侧边栏包含三个工具分组和一个设置分组：

- **SQL 工具**：TSV → SQL IN
- **图片工具**：图片 → Base64
- **数据工具**：JSON 格式化
- **通用设置**：菜单位置（左/右侧）、主题（白色/黑色/跟随系统）

当前项目没有为工具定义统一 trait，工具通过 `ToolId`、`Entity<T>` 和 `match` 显式注册。这种方式对当前规模足够直观：新增工具时需要在编译期完成所有注册，遗漏分支时 Rust 编译器也会给出提示。

## 5. 单个工具的基本结构

每个工具通常由三部分组成。

### 5.1 状态结构体

结构体保存界面需要长期保留的状态，例如输入框实体、转换结果、文件路径、加载状态和错误信息。

```rust
/// JSON 格式化工具的界面状态。
pub struct JsonFormatterTool {
    /// 输入框由 GPUI 管理，因此保存为 Entity<InputState>。
    input_state: Entity<InputState>,
    /// 保存最近一次格式化结果，重新渲染时继续显示。
    output: String,
    /// 解析失败时保存适合展示给用户的错误信息。
    error: Option<String>,
}
```

不要把只在一次函数调用中使用的临时变量放入结构体。只有需要跨事件或跨渲染保留的数据才应成为字段。

### 5.2 构造函数

工具通过 `new(window, cx)` 创建初始状态。需要使用窗口或 GPUI 上下文创建的控件状态，也应在这里初始化。

```rust
impl JsonFormatterTool {
    /// 创建输入框，并设置工具首次打开时显示的默认内容。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .placeholder("在此粘贴 JSON…")
        });

        Self {
            input_state,
            output: String::new(),
            error: None,
        }
    }
}
```

### 5.3 Render 实现

实现 GPUI 的 `Render` trait 后，工具就可以转换为界面元素。`render` 的主要职责是：

- 根据当前状态构建控件。
- 为按钮等控件注册事件回调。
- 在回调中读取输入并更新状态。
- 根据成功、失败或加载状态展示不同内容。

事件回调更新工具实体时，使用以下模式：

```rust
let tool = cx.entity();

// 点击后修改实体状态，并通知框架重新渲染。
tool.update(cx, |tool, cx| {
    tool.output = "新的输出内容".to_string();
    tool.error = None;
    cx.notify();
});
```

gpui-component 的点击处理器可能被调用多次，因此闭包实现的是 `Fn`。将 `String`、`Entity` 等非 `Copy` 值传入更深层闭包或剪贴板前，通常需要调用 `.clone()`，避免把捕获值永久移出点击回调。

## 6. 如何新增一个工具

以下以“JSON 格式化”工具为例。简单工具应新建独立文件 `src/tools/json_formatter.rs`。

### 第一步：创建工具文件

在新文件中定义状态结构体、构造函数、纯业务函数和 `Render` 实现：

```rust
pub struct JsonFormatterTool {
    // 保存输入、输出、错误和其他需要跨渲染保留的状态。
}

impl JsonFormatterTool {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 初始化输入控件和默认状态。
    }

    fn format_json(raw: &str) -> Result<String, JsonFormatterError> {
        // 只处理 JSON 格式化，不直接操作界面。
    }
}

impl Render for JsonFormatterTool {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // 构建输入区、操作按钮、错误提示和输出区。
    }
}
```

建议把“解析和转换”写成不依赖 GPUI 的纯函数。这样业务逻辑更容易编写单元测试，也不会和界面事件处理混在一起。

### 第二步：声明模块和工具 ID

修改 `src/tools/mod.rs`：

```rust
pub mod json_formatter;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolId {
    TsvToSql,
    ImageToBase64,
    JsonFormatter,
}
```

这里沿用项目现有的 `derive` 宏，让编译器自动生成 `Clone`、`Copy`、`PartialEq` 和 `Eq` 实现：

- `Copy` 和 `Clone` 让较小的 `ToolId` 可以方便地传递。
- `PartialEq` 和 `Eq` 用于判断菜单项是否为当前选中项。
- 相比手写四个 trait 的实现，派生宏能减少重复代码，也不容易写错。

如果要引入项目此前未使用的新宏，必须先说明为什么使用、宏能解决什么问题、相比普通函数或 trait 的收益，以及可读性和调试方面的代价。

### 第三步：在应用状态中创建工具实体

修改 `src/app.rs`，先导入工具类型：

```rust
use crate::tools::{
    image_to_base64::ImageTool,
    json_formatter::JsonFormatterTool,
    tsv_to_sql::TsvTool,
    ToolId,
};
```

然后在 `DevToolsApp` 中增加字段：

```rust
pub struct DevToolsApp {
    active: ToolId,
    tsv: Entity<TsvTool>,
    image: Entity<ImageTool>,
    json_formatter: Entity<JsonFormatterTool>,
}
```

在 `DevToolsApp::new` 中创建该实体：

```rust
json_formatter: cx.new(|cx| JsonFormatterTool::new(window, cx)),
```

实体应只创建一次并保存在 `DevToolsApp` 中。切换菜单时复用同一个实体，用户先前输入的内容才不会因为切换工具而丢失。

### 第四步：添加菜单项

在 `DevToolsApp::render` 中为新工具创建用于点击事件的根实体副本：

```rust
let select_json_formatter = root.clone();
```

将菜单项放入合适的 `SidebarGroup`，例如“JSON 工具”：

```rust
SidebarMenuItem::new("JSON 格式化")
    .active(self.active == ToolId::JsonFormatter)
    .on_click(move |_, _, cx| {
        select_json_formatter.update(cx, |app, cx| {
            app.active = ToolId::JsonFormatter;
            cx.notify();
        });
    })
```

每个按钮和可交互元素都应使用在整个界面中唯一、含义明确的 ID，例如 `json-format`、`json-copy`，避免不同工具之间发生元素 ID 冲突。

### 第五步：添加渲染路由

在面板选择的 `match` 中增加分支：

```rust
let panel: AnyElement = match self.active {
    ToolId::TsvToSql => self.tsv.clone().into_any_element(),
    ToolId::ImageToBase64 => self.image.clone().into_any_element(),
    ToolId::JsonFormatter => self.json_formatter.clone().into_any_element(),
};
```

`AnyElement` 用来统一不同工具渲染出的具体元素类型，使它们可以由同一个 `match` 返回。

### 第六步：添加测试和文档

- 为纯转换函数添加成功输入、空输入、无效输入和边界情况测试。
- 如果增加依赖，同步修改 `Cargo.toml`、`Cargo.lock` 和 `README.md`。
- 如果开发、运行或调试命令发生变化，同步修改 `README.md`。
- 使用 `thiserror` 定义工具的错误类型，不新增 `anyhow` 用法。
- 在关键操作、失败位置和耗时任务完成时添加必要日志，但不要记录完整 JSON、Base64 或其他敏感数据。

### 第七步：验证

在项目根目录运行：

```bash
cargo fmt --all
cargo check
cargo test
cargo clippy --all-targets
cargo run
```

除自动检查外，还应手动确认：菜单能够切换、新工具状态能够保留、成功结果正确、错误提示清晰、复制等交互能够正常工作。

## 7. 同步任务与后台任务

TSV 转 SQL 只处理少量文本，可以在按钮回调中同步执行。图片读取可能耗时，因此 `ImageTool` 使用后台执行器：

1. `cx.prompt_for_paths(...)` 异步等待用户选取文件。
2. `cx.spawn(...)` 启动与应用上下文关联的异步任务。
3. `cx.background_executor().spawn(...)` 在后台读取文件，避免阻塞界面线程。
4. 后台任务完成后，通过 `cx.update(...)` 回到应用上下文。
5. 使用 `tool.update(...)` 更新工具状态，再调用 `cx.notify()` 触发渲染。

选择执行方式时遵循以下原则：

- 很快完成的字符串转换可以同步执行。
- 文件读写、大数据处理或其他可能卡住界面的操作放到后台执行。
- 后台任务开始时设置加载状态，完成或失败时都要清除加载状态。
- 任务失败时更新可见错误状态，并记录一次包含非敏感上下文的错误日志。
- 不要在后台线程直接修改 GPUI 实体；应通过应用上下文安全地更新。

## 8. 错误处理与日志

项目使用 `thiserror` 定义所有自定义错误类型，集中在 `src/error.rs` 中的 `AppError` 枚举。`anyhow` 仅保留在 gpui 的 `AssetSource` trait 实现边界处（该 trait 签名要求返回 `anyhow::Result`），不在业务代码中直接使用。

为新工具设计错误时：

- 为解析失败、输入无效、文件读取失败等不同原因定义明确的错误变体。
- 使用 `thiserror` 保留底层错误来源和适合用户理解的错误信息。
- 在界面边界把内部错误转换为用户提示。
- 在能够处理错误的边界记录一次日志，避免每一层都重复记录同一个错误。
- 日志可以记录工具名、操作名、数据类型、数据长度、耗时和错误链。
- 日志不得记录完整输入、图片 Base64、密码、令牌或个人信息。

### 8.1 日志实现

项目使用 `log` + `env_logger`，日志输出到 `stderr`，由环境变量 `RUST_LOG` 控制级别（未设置时默认 `info`）。

选择这套组合的理由：

- **`log` 是门面（facade）而非实现**：代码只依赖 `log::info!` 等接口，不绑定任何具体后端。这样以后想换成文件、网络或其他后端时，改 `Cargo.toml` 和初始化一处即可，业务代码不用动。
- **`env_logger` 是轻量实现**：只依赖标准库，无额外运行时依赖，启动即按 `RUST_LOG` 过滤，非常适合这种桌面工具排查问题（例如 `RUST_LOG=debug` 看全量、`RUST_LOG=tool.image=debug` 只看图片工具）。
- **与 gpui 生态一致**：gpui 官方示例即用 `env_logger::init()` + `log`，风格统一。

关于「日志 API 使用宏」的说明（项目规则要求解释宏的收益与代价）：

- 收益：`log::info!` 这类宏在编译期就能做**惰性格式化**——只有当日志级别确实会输出时才执行格式化与字符串分配，避免无关级别下白白构造字符串；调用点也最直观。
- 代价：宏会跨模块展开，报错位置偶尔不够精确；同时「级别过滤」发生在运行时，宏本身仍会把参数求值（但不会格式化）。对当前规模这些代价可忽略，收益更大，因此选用宏接口。

日志落点约定（当前代码已遵循）：

- 在**关键操作完成**、**失败位置**、**耗时任务结束**时各记录一次，例如窗口创建、工具切换、TSV 转换、图片读取成功/失败、复制到剪贴板。
- 只记录工具名、操作、数据长度、耗时、错误链等非敏感信息；**不记录**完整输入、图片 Base64、密码、令牌或个人信息。
- 不在 `render` 里打日志——`render` 每次状态变化都会执行，属于高频路径，打日志会刷屏且无意义。

## 9. 静态资源

`FsAssetSource` 通过文件系统路径读取资源。当前图片预览把用户选择的路径交给 `img(...)`，GPUI 会通过已注册的资源加载器读取内容。

项目自带图标等固定资源应放在 `assets/` 下，并使用稳定的相对路径管理。不要把生成目录 `target/` 中的文件当作应用资源，因为清理或重新编译时其中内容可能消失。

macOS Dock 图标的源文件是 `assets/icons/app-icon-1024.png`，应用包使用 `assets/icons/AppIcon.icns`。`packaging/macos/Info.plist` 通过 `CFBundleIconFile` 声明图标，`justfile` 的 `package` recipe 在打包时将图标复制到 `.app/Contents/Resources/`。

## 10. 当前架构的扩展方向

随着工具数量增加，可以考虑以下改进，但无需在工具较少时提前复杂化：

- 将菜单元数据集中管理，减少新增工具时修改多处重复代码。
- ~~为菜单位置、主题等全局设置增加独立的设置状态和持久化模块。~~ 已完成（`settings.rs` + `theme.rs`）。
- ~~将复杂工具拆成 `state`、`logic`、`view` 和 `error` 等子模块。~~ 已完成（`error.rs`）；后续复杂工具可继续沿用此模式。
- 抽象统一工具 trait，减少 `match` 路由的重复代码。
- 抽取可复用的输入区、输出区、复制按钮和错误提示组件。
- 为转换类工具建立统一的测试组织方式。

无论如何扩展，都应保持入口只负责组装、业务逻辑可独立测试、一个功能对应一个清晰文件或目录。
