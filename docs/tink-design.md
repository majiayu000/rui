# Tink - Terminal Ink for Rust

> A GPU-less, terminal-native UI framework inspired by [ink](https://github.com/vadimdemedes/ink)

## 一、API 风格对比分析

### ink 核心功能清单

| 功能 | 描述 | 优先级 |
|------|------|--------|
| 声明式组件树 | JSX 语法定义 UI 结构 | P0 |
| Flexbox 布局 | Yoga 引擎实现 | P0 |
| 状态管理 | useState, useReducer | P0 |
| 副作用 | useEffect | P0 |
| 输入处理 | useInput hook | P0 |
| Context | 跨组件状态共享 | P0 |
| 条件渲染 | if/else 控制显示 | P0 |
| 列表渲染 | 动态生成组件列表 | P0 |
| 文本样式 | 颜色、加粗、下划线等 | P0 |
| 焦点管理 | useFocus, Tab 切换 | P1 |
| Static 组件 | 永久渲染（日志等） | P1 |
| 屏幕阅读器 | 无障碍支持 | P2 |

---

### 方案 A: 声明式宏 `rsx!`

```rust
// 示例代码
fn counter() -> Element {
    let count = use_signal(|| 0);

    rsx! {
        Box(flex_direction: Column, padding: 1) {
            Text(color: Green, bold: true) {
                "Count: {count}"
            }
            Box(flex_direction: Row, gap: 1) {
                Text { "[j] -1" }
                Text { "[k] +1" }
            }
        }
    }
}
```

#### 功能实现能力

| ink 功能 | 实现方式 | 难度 | 完整度 |
|---------|---------|------|--------|
| 组件树 | 宏直接生成 | ⭐ | 100% |
| Props | 宏参数传递 | ⭐ | 100% |
| 状态管理 | use_signal (类 Dioxus) | ⭐⭐⭐ | 95% |
| useEffect | use_effect hook | ⭐⭐⭐ | 95% |
| 条件渲染 | 宏内 `if {}` 块 | ⭐⭐ | 100% |
| 列表渲染 | 宏内 `for item in list {}` | ⭐⭐ | 100% |
| Context | use_context / provide_context | ⭐⭐⭐ | 90% |
| useInput | use_input hook | ⭐⭐ | 100% |
| 嵌套组件 | 宏内调用其他组件 | ⭐⭐ | 100% |
| 动态子组件 | `{children}` 插槽 | ⭐⭐ | 95% |

#### 性能分析

```
编译时:
┌─────────────────────────────────────────────────────┐
│  rsx! 宏                                            │
│  ├── 解析 DSL 语法                                  │
│  ├── 生成 Rust AST                                  │
│  └── 零运行时开销（完全展开为普通代码）              │
└─────────────────────────────────────────────────────┘

运行时:
┌─────────────────────────────────────────────────────┐
│  展开后的代码                                        │
│  ├── 直接构造 Element 结构体                        │
│  ├── 无额外 boxing/动态分发                         │
│  └── 内联优化友好                                   │
└─────────────────────────────────────────────────────┘
```

| 指标 | 评分 | 说明 |
|------|------|------|
| 编译时间 | ⭐⭐⭐ | 宏展开增加编译时间 |
| 运行时性能 | ⭐⭐⭐⭐⭐ | 零抽象开销 |
| 内存占用 | ⭐⭐⭐⭐⭐ | 静态结构，无动态分配 |
| 代码生成量 | ⭐⭐⭐ | 宏展开代码较多 |

#### 优点
- ✅ 语法最接近 ink/JSX，学习成本低
- ✅ 编译时类型检查
- ✅ 零运行时开销
- ✅ IDE 支持好（rust-analyzer）
- ✅ 成熟方案（Dioxus, Leptos, Yew 验证）

#### 缺点
- ❌ 宏调试困难
- ❌ 编译时间增加
- ❌ 宏实现复杂度高
- ❌ 错误信息可能不友好

---

### 方案 B: Builder 模式

```rust
// 示例代码
fn counter(cx: &mut Context) -> Element {
    let count = cx.use_signal(|| 0);

    Box::new()
        .flex_direction(FlexDirection::Column)
        .padding(1)
        .child(
            Text::new(format!("Count: {}", count.get()))
                .color(Color::Green)
                .bold()
        )
        .child(
            Box::new()
                .flex_direction(FlexDirection::Row)
                .gap(1)
                .child(Text::new("[j] -1"))
                .child(Text::new("[k] +1"))
        )
        .build()
}
```

#### 功能实现能力

| ink 功能 | 实现方式 | 难度 | 完整度 |
|---------|---------|------|--------|
| 组件树 | Builder 链式调用 | ⭐ | 100% |
| Props | Builder 方法 | ⭐ | 100% |
| 状态管理 | Context 参数传递 | ⭐⭐⭐ | 95% |
| useEffect | cx.use_effect() | ⭐⭐⭐ | 95% |
| 条件渲染 | 原生 Rust if/else | ⭐ | 100% |
| 列表渲染 | 原生 Rust iterator | ⭐ | 100% |
| Context | cx.use_context() | ⭐⭐⭐ | 90% |
| useInput | cx.use_input() | ⭐⭐ | 100% |
| 嵌套组件 | .child(component()) | ⭐ | 100% |
| 动态子组件 | .children(vec![...]) | ⭐ | 100% |

#### 性能分析

```
编译时:
┌─────────────────────────────────────────────────────┐
│  Builder 模式                                        │
│  ├── 无宏处理                                        │
│  ├── 标准 Rust 编译                                  │
│  └── 增量编译友好                                    │
└─────────────────────────────────────────────────────┘

运行时:
┌─────────────────────────────────────────────────────┐
│  Builder 执行                                        │
│  ├── 链式调用（可内联优化）                          │
│  ├── 中间状态分配（可优化消除）                      │
│  └── 类型安全但略有运行时开销                        │
└─────────────────────────────────────────────────────┘
```

| 指标 | 评分 | 说明 |
|------|------|------|
| 编译时间 | ⭐⭐⭐⭐⭐ | 最快，无宏处理 |
| 运行时性能 | ⭐⭐⭐⭐ | 链式调用有微小开销 |
| 内存占用 | ⭐⭐⭐⭐ | Builder 中间状态 |
| 代码生成量 | ⭐⭐⭐⭐⭐ | 代码即所见 |

#### 优点
- ✅ 实现简单，无宏复杂度
- ✅ 调试友好，错误信息清晰
- ✅ 编译最快
- ✅ 原生 Rust 控制流（if/for）
- ✅ IDE 支持完美

#### 缺点
- ❌ 代码冗长，嵌套深时可读性差
- ❌ 语法与 ink/JSX 差异大
- ❌ 深层嵌套缩进问题

---

### 方案 C: HTML-like 宏 `view!`

```rust
// 示例代码
fn counter() -> Element {
    let count = use_signal(|| 0);

    view! {
        <Box flex_direction={Column} padding={1}>
            <Text color={Green} bold=true>
                "Count: " {count}
            </Text>
            <Box flex_direction={Row} gap={1}>
                <Text>"[j] -1"</Text>
                <Text>"[k] +1"</Text>
            </Box>
        </Box>
    }
}
```

#### 功能实现能力

与方案 A 基本相同，仅语法不同。

| 指标 | 评分 | 说明 |
|------|------|------|
| 编译时间 | ⭐⭐⭐ | 宏展开开销 |
| 运行时性能 | ⭐⭐⭐⭐⭐ | 零抽象开销 |
| 内存占用 | ⭐⭐⭐⭐⭐ | 静态结构 |
| 代码生成量 | ⭐⭐⭐ | 宏展开代码较多 |

#### 优点
- ✅ 语法最接近 HTML/JSX
- ✅ 前端开发者熟悉
- ✅ 视觉结构清晰

#### 缺点
- ❌ 宏实现更复杂（需解析 XML-like 语法）
- ❌ 与 Rust 语法风格不一致
- ❌ 尖括号与泛型冲突

---

## 二、综合对比矩阵

### 功能完整度对比

| 功能 | 方案 A (rsx!) | 方案 B (Builder) | 方案 C (view!) |
|------|---------------|------------------|----------------|
| 组件树声明 | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| 条件渲染 | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| 列表渲染 | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| 状态管理 | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| 动态组件 | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| 代码可读性 | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |

### 性能对比

| 指标 | 方案 A (rsx!) | 方案 B (Builder) | 方案 C (view!) |
|------|---------------|------------------|----------------|
| 编译速度 | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| 运行时速度 | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| 内存效率 | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| 二进制大小 | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |

### 开发体验对比

| 指标 | 方案 A (rsx!) | 方案 B (Builder) | 方案 C (view!) |
|------|---------------|------------------|----------------|
| 学习曲线 | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| 错误信息 | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| IDE 支持 | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| 调试体验 | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| 社区熟悉度 | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ |

---

## 三、推荐方案

### 🏆 推荐: 方案 A (rsx! 宏) + 方案 B (Builder) 混合

**理由：**

1. **rsx! 宏作为主要 API** - 提供最佳开发体验，与 ink 风格一致
2. **Builder 作为底层实现** - 宏展开为 Builder 调用，方便调试
3. **用户可选择** - 不喜欢宏的用户可直接用 Builder

```rust
// 用户视角 - 简洁的 rsx! 宏
fn app() -> Element {
    rsx! {
        Box(padding: 1) {
            Text(bold: true) { "Hello" }
        }
    }
}

// 宏展开后 - Builder 调用
fn app() -> Element {
    Box::new()
        .padding(1)
        .child(Text::new("Hello").bold())
        .into_element()
}

// 高级用户 - 直接用 Builder（完全控制）
fn app() -> Element {
    let mut container = Box::new().padding(1);
    if some_condition {
        container = container.child(Text::new("Conditional"));
    }
    for item in items {
        container = container.child(render_item(item));
    }
    container.into_element()
}
```

### 架构决策

```
┌─────────────────────────────────────────────────────────────┐
│                      用户 API 层                            │
│  ┌─────────────────┐    ┌─────────────────────────────┐    │
│  │   rsx! 宏       │    │     Builder API             │    │
│  │   (推荐)        │    │     (可选)                  │    │
│  └────────┬────────┘    └──────────────┬──────────────┘    │
│           │                            │                    │
│           └────────────┬───────────────┘                    │
│                        ▼                                    │
├─────────────────────────────────────────────────────────────┤
│                    Element 树                               │
├─────────────────────────────────────────────────────────────┤
│                  Reconciler (Diff)                          │
├─────────────────────────────────────────────────────────────┤
│                  Taffy (Flexbox)                            │
├─────────────────────────────────────────────────────────────┤
│                  Output Buffer                              │
├─────────────────────────────────────────────────────────────┤
│                  Crossterm (Terminal)                       │
└─────────────────────────────────────────────────────────────┘
```

---

## 四、ink 完整功能映射

### 组件映射

| ink 组件 | Tink 组件 | 说明 |
|----------|-----------|------|
| `<Box>` | `Box` | Flexbox 容器 |
| `<Text>` | `Text` | 文本渲染 |
| `<Newline>` | `Newline` | 换行 |
| `<Spacer>` | `Spacer` | Flex 空白 |
| `<Static>` | `Static` | 永久输出 |
| `<Transform>` | `Transform` | 文本变换 |

### Hooks 映射

| ink Hook | Tink Hook | 说明 |
|----------|-----------|------|
| `useState` | `use_signal` | 状态管理 |
| `useReducer` | `use_reducer` | 复杂状态 |
| `useEffect` | `use_effect` | 副作用 |
| `useInput` | `use_input` | 键盘输入 |
| `useFocus` | `use_focus` | 焦点管理 |
| `useApp` | `use_app` | App 上下文 |
| `useStdin` | `use_stdin` | 标准输入 |
| `useStdout` | `use_stdout` | 标准输出 |

### 样式映射

| ink 样式 | Tink 样式 | 说明 |
|----------|-----------|------|
| `flexDirection` | `flex_direction` | 布局方向 |
| `flexGrow` | `flex_grow` | 扩展比例 |
| `flexShrink` | `flex_shrink` | 收缩比例 |
| `padding` | `padding` | 内边距 |
| `margin` | `margin` | 外边距 |
| `gap` | `gap` | 间距 |
| `width` | `width` | 宽度 |
| `height` | `height` | 高度 |
| `borderStyle` | `border_style` | 边框样式 |
| `borderColor` | `border_color` | 边框颜色 |

---

## 五、技术实现规格

### 5.1 项目结构

```
tink/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # 库入口
│   ├── prelude.rs             # 常用导出
│   │
│   ├── core/                  # 核心类型
│   │   ├── mod.rs
│   │   ├── element.rs         # Element 定义
│   │   ├── node.rs            # DOM 节点
│   │   ├── style.rs           # 样式系统
│   │   ├── color.rs           # 颜色类型
│   │   └── context.rs         # Context 系统
│   │
│   ├── components/            # 内置组件
│   │   ├── mod.rs
│   │   ├── box.rs             # Box 容器
│   │   ├── text.rs            # Text 文本
│   │   ├── newline.rs         # Newline
│   │   ├── spacer.rs          # Spacer
│   │   ├── static.rs          # Static
│   │   └── transform.rs       # Transform
│   │
│   ├── hooks/                 # Hooks 系统
│   │   ├── mod.rs
│   │   ├── use_signal.rs      # 状态管理
│   │   ├── use_effect.rs      # 副作用
│   │   ├── use_reducer.rs     # Reducer
│   │   ├── use_input.rs       # 输入处理
│   │   ├── use_focus.rs       # 焦点管理
│   │   ├── use_context.rs     # Context
│   │   └── use_app.rs         # App 访问
│   │
│   ├── reconciler/            # Diff 算法
│   │   ├── mod.rs
│   │   ├── diff.rs            # Diff 实现
│   │   ├── patch.rs           # Patch 应用
│   │   └── scheduler.rs       # 调度器
│   │
│   ├── layout/                # 布局系统
│   │   ├── mod.rs
│   │   ├── flexbox.rs         # Taffy 集成
│   │   └── measure.rs         # 文本测量
│   │
│   ├── renderer/              # 渲染器
│   │   ├── mod.rs
│   │   ├── output.rs          # Output buffer
│   │   ├── terminal.rs        # Crossterm 集成
│   │   └── styled_char.rs     # 样式字符
│   │
│   ├── input/                 # 输入处理
│   │   ├── mod.rs
│   │   ├── keypress.rs        # 按键解析
│   │   └── event.rs           # 事件类型
│   │
│   ├── app.rs                 # App 主类
│   └── macros.rs              # rsx! 宏定义
│
├── tink-macros/               # 过程宏 crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── rsx.rs             # rsx! 宏实现
│
└── examples/
    ├── hello.rs               # 基础示例
    ├── counter.rs             # 状态示例
    ├── input.rs               # 输入示例
    ├── todo.rs                # 完整应用
    └── dashboard.rs           # 复杂布局
```

### 5.2 核心类型定义

```rust
// === element.rs ===

/// 元素类型
#[derive(Debug, Clone)]
pub enum ElementType {
    Box,
    Text,
    VirtualText,  // 嵌套在 Text 内的文本
    Root,
}

/// 元素节点
#[derive(Debug)]
pub struct Element {
    pub id: ElementId,
    pub element_type: ElementType,
    pub props: Props,
    pub style: Style,
    pub children: Vec<Element>,
    pub text_content: Option<String>,
    pub transform: Option<TextTransform>,
}

/// 元素 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementId(u64);

/// Props 存储
#[derive(Debug, Clone, Default)]
pub struct Props {
    pub aria_label: Option<String>,
    pub aria_hidden: bool,
    // ... 其他属性
}
```

```rust
// === node.rs ===

/// DOM 节点（内部表示）
pub struct DOMNode {
    pub element_type: ElementType,
    pub yoga_node: Option<taffy::NodeId>,
    pub style: Style,
    pub children: Vec<DOMNode>,
    pub text_content: Option<String>,
    pub transform: Option<Box<dyn Fn(&str) -> String>>,
    pub parent: Option<*mut DOMNode>,
}

/// 文本节点
pub struct TextNode {
    pub value: String,
    pub parent: Option<*mut DOMNode>,
}
```

```rust
// === style.rs ===

use taffy::prelude::*;

/// 样式定义
#[derive(Debug, Clone, Default)]
pub struct Style {
    // Display
    pub display: Display,

    // Flexbox
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: Dimension,
    pub align_items: AlignItems,
    pub align_self: AlignSelf,
    pub justify_content: JustifyContent,

    // Spacing
    pub padding: Edges,
    pub margin: Edges,
    pub gap: Gap,

    // Size
    pub width: Dimension,
    pub height: Dimension,
    pub min_width: Dimension,
    pub min_height: Dimension,
    pub max_width: Dimension,
    pub max_height: Dimension,

    // Border
    pub border_style: BorderStyle,
    pub border_color: Option<Color>,
    pub border_dim: bool,
    pub border_top: bool,
    pub border_bottom: bool,
    pub border_left: bool,
    pub border_right: bool,

    // Colors
    pub color: Option<Color>,
    pub background_color: Option<Color>,

    // Text
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub dim: bool,
    pub inverse: bool,
    pub text_wrap: TextWrap,

    // Overflow
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum TextWrap {
    #[default]
    Wrap,
    Truncate,
    TruncateStart,
    TruncateMiddle,
    TruncateEnd,
}

#[derive(Debug, Clone, Copy)]
pub enum BorderStyle {
    Single,
    Double,
    Round,
    Bold,
    SingleDouble,
    DoubleSingle,
    Classic,
    Arrow,
}
```

```rust
// === color.rs ===

/// 颜色类型
#[derive(Debug, Clone, Copy)]
pub enum Color {
    // 基础颜色
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,

    // 亮色
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,

    // 256 色
    Ansi256(u8),

    // RGB
    Rgb(u8, u8, u8),

    // Hex
    Hex(u32),
}

impl Color {
    pub fn hex(hex: &str) -> Self {
        let hex = hex.trim_start_matches('#');
        let value = u32::from_str_radix(hex, 16).unwrap_or(0);
        Color::Hex(value)
    }
}
```

### 5.3 Hooks 系统

```rust
// === hooks/mod.rs ===

use std::cell::RefCell;
use std::rc::Rc;

/// Hook 上下文
thread_local! {
    static HOOK_CONTEXT: RefCell<Option<HookContext>> = RefCell::new(None);
}

pub struct HookContext {
    pub component_id: ComponentId,
    pub hook_index: usize,
    pub hooks: Vec<Box<dyn Any>>,
    pub effects: Vec<Effect>,
    pub scheduler: Rc<RefCell<Scheduler>>,
}

/// 设置当前 hook 上下文
pub fn with_hook_context<F, R>(ctx: &mut HookContext, f: F) -> R
where
    F: FnOnce() -> R,
{
    HOOK_CONTEXT.with(|cell| {
        *cell.borrow_mut() = Some(ctx.clone());
    });
    let result = f();
    HOOK_CONTEXT.with(|cell| {
        *cell.borrow_mut() = None;
    });
    result
}
```

```rust
// === hooks/use_signal.rs ===

use std::cell::RefCell;
use std::rc::Rc;

/// Signal - 响应式状态
pub struct Signal<T> {
    value: Rc<RefCell<T>>,
    subscribers: Rc<RefCell<Vec<Box<dyn Fn()>>>>,
}

impl<T: Clone> Signal<T> {
    pub fn get(&self) -> T {
        self.value.borrow().clone()
    }

    pub fn set(&self, value: T) {
        *self.value.borrow_mut() = value;
        self.notify();
    }

    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        f(&mut self.value.borrow_mut());
        self.notify();
    }

    fn notify(&self) {
        for subscriber in self.subscribers.borrow().iter() {
            subscriber();
        }
    }
}

/// 创建 Signal
pub fn use_signal<T: Clone + 'static>(initial: impl FnOnce() -> T) -> Signal<T> {
    HOOK_CONTEXT.with(|cell| {
        let mut ctx = cell.borrow_mut();
        let ctx = ctx.as_mut().expect("use_signal called outside component");

        let index = ctx.hook_index;
        ctx.hook_index += 1;

        if index >= ctx.hooks.len() {
            let signal = Signal {
                value: Rc::new(RefCell::new(initial())),
                subscribers: Rc::new(RefCell::new(vec![])),
            };
            ctx.hooks.push(Box::new(signal.clone()));
            signal
        } else {
            ctx.hooks[index]
                .downcast_ref::<Signal<T>>()
                .expect("Hook type mismatch")
                .clone()
        }
    })
}
```

```rust
// === hooks/use_effect.rs ===

/// Effect 类型
pub struct Effect {
    pub deps: Option<Vec<Box<dyn Any>>>,
    pub cleanup: Option<Box<dyn FnOnce()>>,
    pub effect: Box<dyn FnOnce() -> Option<Box<dyn FnOnce()>>>,
}

/// 执行副作用
pub fn use_effect<F, D>(effect: F, deps: D)
where
    F: FnOnce() -> Option<Box<dyn FnOnce()>> + 'static,
    D: IntoDeps + 'static,
{
    HOOK_CONTEXT.with(|cell| {
        let mut ctx = cell.borrow_mut();
        let ctx = ctx.as_mut().expect("use_effect called outside component");

        let new_deps = deps.into_deps();
        let index = ctx.hook_index;
        ctx.hook_index += 1;

        let should_run = if index >= ctx.hooks.len() {
            true
        } else {
            let old_deps = ctx.hooks[index]
                .downcast_ref::<Option<Vec<Box<dyn Any>>>>()
                .expect("Hook type mismatch");

            match (old_deps, &new_deps) {
                (None, None) => false,  // 空依赖，只运行一次
                (Some(old), Some(new)) => !deps_equal(old, new),
                _ => true,
            }
        };

        if should_run {
            ctx.effects.push(Effect {
                deps: new_deps,
                cleanup: None,
                effect: Box::new(effect),
            });
        }

        if index >= ctx.hooks.len() {
            ctx.hooks.push(Box::new(new_deps));
        } else {
            ctx.hooks[index] = Box::new(new_deps);
        }
    });
}
```

```rust
// === hooks/use_input.rs ===

use crate::input::{Key, KeyEvent};

/// 输入处理 hook
pub fn use_input<F>(handler: F)
where
    F: Fn(&str, Key) + 'static,
{
    use_input_with_options(handler, UseInputOptions::default());
}

#[derive(Default)]
pub struct UseInputOptions {
    pub is_active: bool,
}

pub fn use_input_with_options<F>(handler: F, options: UseInputOptions)
where
    F: Fn(&str, Key) + 'static,
{
    let handler = Rc::new(handler);

    use_effect(
        move || {
            if !options.is_active {
                return None;
            }

            let handler = handler.clone();

            // 注册输入监听
            INPUT_EMITTER.with(|emitter| {
                emitter.borrow_mut().add_listener(move |event: &KeyEvent| {
                    handler(&event.input, event.key.clone());
                });
            });

            Some(Box::new(|| {
                // 清理监听
            }) as Box<dyn FnOnce()>)
        },
        (options.is_active,),
    );
}

/// Key 类型定义
#[derive(Debug, Clone, Default)]
pub struct Key {
    pub up_arrow: bool,
    pub down_arrow: bool,
    pub left_arrow: bool,
    pub right_arrow: bool,
    pub page_up: bool,
    pub page_down: bool,
    pub home: bool,
    pub end: bool,
    pub return_key: bool,
    pub escape: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub meta: bool,
    pub tab: bool,
    pub backspace: bool,
    pub delete: bool,
}
```

### 5.4 Reconciler 实现

```rust
// === reconciler/diff.rs ===

use crate::core::{Element, ElementId};

/// Diff 操作类型
#[derive(Debug)]
pub enum DiffOp {
    /// 创建新节点
    Create {
        element: Element,
        parent_id: ElementId,
        index: usize,
    },
    /// 更新节点
    Update {
        id: ElementId,
        old_props: Props,
        new_props: Props,
        old_style: Style,
        new_style: Style,
    },
    /// 删除节点
    Remove {
        id: ElementId,
    },
    /// 移动节点
    Move {
        id: ElementId,
        new_parent_id: ElementId,
        new_index: usize,
    },
    /// 更新文本
    UpdateText {
        id: ElementId,
        old_text: String,
        new_text: String,
    },
}

/// 执行 diff
pub fn diff(old_tree: &Element, new_tree: &Element) -> Vec<DiffOp> {
    let mut ops = Vec::new();
    diff_node(old_tree, new_tree, &mut ops);
    ops
}

fn diff_node(old: &Element, new: &Element, ops: &mut Vec<DiffOp>) {
    // 类型不同，完全替换
    if old.element_type != new.element_type {
        ops.push(DiffOp::Remove { id: old.id });
        ops.push(DiffOp::Create {
            element: new.clone(),
            parent_id: old.parent_id.unwrap_or(ElementId::ROOT),
            index: old.index,
        });
        return;
    }

    // Props/Style 变化
    if old.props != new.props || old.style != new.style {
        ops.push(DiffOp::Update {
            id: old.id,
            old_props: old.props.clone(),
            new_props: new.props.clone(),
            old_style: old.style.clone(),
            new_style: new.style.clone(),
        });
    }

    // 文本内容变化
    if old.text_content != new.text_content {
        if let (Some(old_text), Some(new_text)) = (&old.text_content, &new.text_content) {
            ops.push(DiffOp::UpdateText {
                id: old.id,
                old_text: old_text.clone(),
                new_text: new_text.clone(),
            });
        }
    }

    // Diff 子节点
    diff_children(&old.children, &new.children, old.id, ops);
}

fn diff_children(
    old_children: &[Element],
    new_children: &[Element],
    parent_id: ElementId,
    ops: &mut Vec<DiffOp>,
) {
    let mut old_keyed: HashMap<Option<String>, &Element> = HashMap::new();
    let mut old_unkeyed: Vec<&Element> = Vec::new();

    for child in old_children {
        if let Some(key) = &child.key {
            old_keyed.insert(Some(key.clone()), child);
        } else {
            old_unkeyed.push(child);
        }
    }

    let mut unkeyed_index = 0;

    for (index, new_child) in new_children.iter().enumerate() {
        let old_child = if let Some(key) = &new_child.key {
            old_keyed.remove(&Some(key.clone()))
        } else {
            let child = old_unkeyed.get(unkeyed_index).copied();
            unkeyed_index += 1;
            child
        };

        match old_child {
            Some(old) => diff_node(old, new_child, ops),
            None => {
                ops.push(DiffOp::Create {
                    element: new_child.clone(),
                    parent_id,
                    index,
                });
            }
        }
    }

    // 移除多余的旧节点
    for (_, old) in old_keyed {
        ops.push(DiffOp::Remove { id: old.id });
    }
    for old in old_unkeyed.into_iter().skip(new_children.len()) {
        ops.push(DiffOp::Remove { id: old.id });
    }
}
```

### 5.5 渲染器实现

```rust
// === renderer/output.rs ===

use crate::core::Color;

/// 样式字符
#[derive(Debug, Clone)]
pub struct StyledChar {
    pub char: char,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub dim: bool,
    pub inverse: bool,
}

/// 输出缓冲区
pub struct Output {
    pub width: u16,
    pub height: u16,
    operations: Vec<Operation>,
    clip_stack: Vec<ClipRegion>,
}

#[derive(Debug)]
enum Operation {
    Write {
        x: u16,
        y: u16,
        text: String,
        style: TextStyle,
    },
    Clip(ClipRegion),
    Unclip,
}

#[derive(Debug, Clone)]
struct ClipRegion {
    x1: u16,
    y1: u16,
    x2: u16,
    y2: u16,
}

impl Output {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            operations: Vec::new(),
            clip_stack: Vec::new(),
        }
    }

    pub fn write(&mut self, x: u16, y: u16, text: &str, style: TextStyle) {
        self.operations.push(Operation::Write {
            x,
            y,
            text: text.to_string(),
            style,
        });
    }

    pub fn clip(&mut self, region: ClipRegion) {
        self.clip_stack.push(region.clone());
        self.operations.push(Operation::Clip(region));
    }

    pub fn unclip(&mut self) {
        self.clip_stack.pop();
        self.operations.push(Operation::Unclip);
    }

    /// 生成最终输出
    pub fn get(&self) -> String {
        // 初始化 2D 网格
        let mut grid: Vec<Vec<StyledChar>> = vec![
            vec![StyledChar::default(); self.width as usize];
            self.height as usize
        ];

        let mut clip_stack: Vec<ClipRegion> = Vec::new();

        for op in &self.operations {
            match op {
                Operation::Write { x, y, text, style } => {
                    self.write_to_grid(&mut grid, *x, *y, text, style, &clip_stack);
                }
                Operation::Clip(region) => {
                    clip_stack.push(region.clone());
                }
                Operation::Unclip => {
                    clip_stack.pop();
                }
            }
        }

        // 转换为字符串
        self.grid_to_string(&grid)
    }

    fn write_to_grid(
        &self,
        grid: &mut [Vec<StyledChar>],
        x: u16,
        y: u16,
        text: &str,
        style: &TextStyle,
        clip_stack: &[ClipRegion],
    ) {
        let mut col = x as usize;
        let row = y as usize;

        if row >= grid.len() {
            return;
        }

        for ch in text.chars() {
            if ch == '\n' {
                break;
            }

            if col >= grid[row].len() {
                break;
            }

            // 检查裁剪区域
            if let Some(clip) = clip_stack.last() {
                if col < clip.x1 as usize
                    || col >= clip.x2 as usize
                    || row < clip.y1 as usize
                    || row >= clip.y2 as usize
                {
                    col += 1;
                    continue;
                }
            }

            grid[row][col] = StyledChar {
                char: ch,
                fg: style.color,
                bg: style.background_color,
                bold: style.bold,
                italic: style.italic,
                underline: style.underline,
                strikethrough: style.strikethrough,
                dim: style.dim,
                inverse: style.inverse,
            };

            col += 1;
        }
    }

    fn grid_to_string(&self, grid: &[Vec<StyledChar>]) -> String {
        use crossterm::style::{Attribute, Color as CtColor, SetAttribute, SetForegroundColor, SetBackgroundColor};

        let mut result = String::new();

        for (row_idx, row) in grid.iter().enumerate() {
            let mut current_style = TextStyle::default();

            for cell in row {
                // 应用样式变化
                let new_style = TextStyle::from(cell);
                if new_style != current_style {
                    result.push_str(&self.style_to_ansi(&new_style));
                    current_style = new_style;
                }

                result.push(cell.char);
            }

            // 重置样式并换行
            result.push_str("\x1b[0m");
            if row_idx < grid.len() - 1 {
                result.push('\n');
            }
        }

        result
    }

    fn style_to_ansi(&self, style: &TextStyle) -> String {
        let mut codes = Vec::new();

        if style.bold {
            codes.push("1");
        }
        if style.dim {
            codes.push("2");
        }
        if style.italic {
            codes.push("3");
        }
        if style.underline {
            codes.push("4");
        }
        if style.inverse {
            codes.push("7");
        }
        if style.strikethrough {
            codes.push("9");
        }

        if let Some(color) = &style.color {
            codes.push(&self.color_to_fg_code(color));
        }

        if let Some(color) = &style.background_color {
            codes.push(&self.color_to_bg_code(color));
        }

        if codes.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", codes.join(";"))
        }
    }
}
```

```rust
// === renderer/terminal.rs ===

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use std::io::{stdout, Write};

pub struct Terminal {
    previous_output: String,
    previous_line_count: usize,
}

impl Terminal {
    pub fn new() -> Self {
        Self {
            previous_output: String::new(),
            previous_line_count: 0,
        }
    }

    pub fn enter(&mut self) -> std::io::Result<()> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, Hide)?;
        Ok(())
    }

    pub fn exit(&mut self) -> std::io::Result<()> {
        execute!(stdout(), Show, LeaveAlternateScreen)?;
        disable_raw_mode()?;
        Ok(())
    }

    pub fn render(&mut self, output: &str) -> std::io::Result<()> {
        let mut stdout = stdout();

        // 清除之前的输出
        if self.previous_line_count > 0 {
            for _ in 0..self.previous_line_count {
                execute!(stdout, crossterm::cursor::MoveUp(1))?;
                execute!(stdout, Clear(ClearType::CurrentLine))?;
            }
        }

        // 写入新输出
        write!(stdout, "{}", output)?;
        stdout.flush()?;

        self.previous_output = output.to_string();
        self.previous_line_count = output.lines().count();

        Ok(())
    }

    pub fn size(&self) -> std::io::Result<(u16, u16)> {
        crossterm::terminal::size()
    }
}
```

### 5.6 App 主类

```rust
// === app.rs ===

use std::time::{Duration, Instant};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};

pub struct App<F>
where
    F: Fn() -> Element,
{
    component: F,
    terminal: Terminal,
    reconciler: Reconciler,
    layout_engine: LayoutEngine,
    output: Output,
    options: AppOptions,
}

#[derive(Default)]
pub struct AppOptions {
    pub fps: u32,
    pub exit_on_ctrl_c: bool,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            fps: 30,
            exit_on_ctrl_c: true,
        }
    }
}

impl<F> App<F>
where
    F: Fn() -> Element,
{
    pub fn new(component: F) -> Self {
        Self::with_options(component, AppOptions::default())
    }

    pub fn with_options(component: F, options: AppOptions) -> Self {
        Self {
            component,
            terminal: Terminal::new(),
            reconciler: Reconciler::new(),
            layout_engine: LayoutEngine::new(),
            output: Output::new(0, 0),
            options,
        }
    }

    pub fn run(&mut self) -> std::io::Result<()> {
        self.terminal.enter()?;

        let frame_duration = Duration::from_millis(1000 / self.options.fps as u64);
        let mut last_render = Instant::now();

        loop {
            // 处理输入
            if event::poll(Duration::from_millis(10))? {
                if let Event::Key(key_event) = event::read()? {
                    // Ctrl+C 退出
                    if self.options.exit_on_ctrl_c
                        && key_event.code == KeyCode::Char('c')
                        && key_event.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }

                    // 分发输入事件
                    self.dispatch_input(key_event);
                }
            }

            // 节流渲染
            let now = Instant::now();
            if now.duration_since(last_render) >= frame_duration {
                self.render()?;
                last_render = now;
            }
        }

        self.terminal.exit()?;
        Ok(())
    }

    fn render(&mut self) -> std::io::Result<()> {
        // 1. 执行组件函数，生成 Element 树
        let new_tree = (self.component)();

        // 2. Diff
        let ops = self.reconciler.diff(&new_tree);

        // 3. 应用 Patch
        self.reconciler.apply(ops);

        // 4. 布局计算
        let (width, height) = self.terminal.size()?;
        self.layout_engine.compute(&self.reconciler.tree, width, height);

        // 5. 渲染到 Output buffer
        self.output = Output::new(width, height);
        render_node_to_output(&self.reconciler.tree, &mut self.output, 0, 0);

        // 6. 输出到终端
        let output_str = self.output.get();
        self.terminal.render(&output_str)?;

        Ok(())
    }

    fn dispatch_input(&self, key_event: crossterm::event::KeyEvent) {
        let key = Key::from(key_event);
        let input = key_to_string(&key_event);

        INPUT_EMITTER.with(|emitter| {
            emitter.borrow().emit(KeyEvent { input, key });
        });
    }
}

/// 渲染函数
pub fn render<F>(component: F) -> std::io::Result<()>
where
    F: Fn() -> Element,
{
    App::new(component).run()
}
```

### 5.7 rsx! 宏实现

```rust
// === tink-macros/src/rsx.rs ===

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse::Parse, parse::ParseStream, Expr, Ident, Token};

pub fn rsx_impl(input: TokenStream) -> TokenStream {
    let nodes = syn::parse2::<RsxNodes>(input).expect("Failed to parse rsx");
    generate_element(&nodes.nodes[0])
}

struct RsxNodes {
    nodes: Vec<RsxNode>,
}

enum RsxNode {
    Element {
        name: Ident,
        props: Vec<(Ident, Expr)>,
        children: Vec<RsxNode>,
    },
    Text(String),
    Expr(Expr),
    If {
        condition: Expr,
        then_branch: Vec<RsxNode>,
        else_branch: Option<Vec<RsxNode>>,
    },
    For {
        binding: Ident,
        iter: Expr,
        body: Vec<RsxNode>,
    },
}

fn generate_element(node: &RsxNode) -> TokenStream {
    match node {
        RsxNode::Element { name, props, children } => {
            let name_str = name.to_string();

            let prop_setters: Vec<TokenStream> = props
                .iter()
                .map(|(key, value)| {
                    let key_str = key.to_string();
                    quote! { .#key(#value) }
                })
                .collect();

            let child_exprs: Vec<TokenStream> = children
                .iter()
                .map(|child| generate_element(child))
                .collect();

            match name_str.as_str() {
                "Box" => quote! {
                    ::tink::components::Box::new()
                        #(#prop_setters)*
                        #(.child(#child_exprs))*
                        .into_element()
                },
                "Text" => {
                    if children.is_empty() {
                        quote! {
                            ::tink::components::Text::new("")
                                #(#prop_setters)*
                                .into_element()
                        }
                    } else {
                        let text_content = &children[0];
                        let text_expr = generate_text_content(text_content);
                        quote! {
                            ::tink::components::Text::new(#text_expr)
                                #(#prop_setters)*
                                .into_element()
                        }
                    }
                },
                _ => quote! {
                    #name::new()
                        #(#prop_setters)*
                        #(.child(#child_exprs))*
                        .into_element()
                },
            }
        }
        RsxNode::Text(text) => {
            quote! { #text.to_string() }
        }
        RsxNode::Expr(expr) => {
            quote! { (#expr).to_string() }
        }
        RsxNode::If { condition, then_branch, else_branch } => {
            let then_exprs: Vec<TokenStream> = then_branch
                .iter()
                .map(|n| generate_element(n))
                .collect();

            if let Some(else_branch) = else_branch {
                let else_exprs: Vec<TokenStream> = else_branch
                    .iter()
                    .map(|n| generate_element(n))
                    .collect();
                quote! {
                    if #condition {
                        vec![#(#then_exprs),*]
                    } else {
                        vec![#(#else_exprs),*]
                    }
                }
            } else {
                quote! {
                    if #condition {
                        vec![#(#then_exprs),*]
                    } else {
                        vec![]
                    }
                }
            }
        }
        RsxNode::For { binding, iter, body } => {
            let body_exprs: Vec<TokenStream> = body
                .iter()
                .map(|n| generate_element(n))
                .collect();

            quote! {
                (#iter).into_iter().map(|#binding| {
                    #(#body_exprs)*
                }).collect::<Vec<_>>()
            }
        }
    }
}

fn generate_text_content(node: &RsxNode) -> TokenStream {
    match node {
        RsxNode::Text(text) => quote! { #text },
        RsxNode::Expr(expr) => quote! { format!("{}", #expr) },
        _ => quote! { "" },
    }
}
```

---

## 六、完整示例

### 6.1 Hello World

```rust
use tink::prelude::*;

fn main() -> std::io::Result<()> {
    render(app)
}

fn app() -> Element {
    rsx! {
        Box(padding: 1, border_style: BorderStyle::Round, border_color: Color::Cyan) {
            Text(color: Color::Green, bold: true) {
                "Hello, Tink! 🎉"
            }
        }
    }
}
```

### 6.2 Counter

```rust
use tink::prelude::*;

fn main() -> std::io::Result<()> {
    render(counter)
}

fn counter() -> Element {
    let count = use_signal(|| 0);

    use_input(move |input, key| {
        match input.as_str() {
            "j" | "down" => count.update(|c| *c -= 1),
            "k" | "up" => count.update(|c| *c += 1),
            "q" => std::process::exit(0),
            _ => {}
        }
    });

    rsx! {
        Box(flex_direction: FlexDirection::Column, padding: 1) {
            Text(bold: true) {
                "Counter: {count}"
            }
            Box(margin_top: 1) {
                Text(dim: true) {
                    "j/k to change, q to quit"
                }
            }
        }
    }
}
```

### 6.3 Todo App

```rust
use tink::prelude::*;

fn main() -> std::io::Result<()> {
    render(todo_app)
}

#[derive(Clone)]
struct Todo {
    id: usize,
    text: String,
    done: bool,
}

fn todo_app() -> Element {
    let todos = use_signal(|| vec![
        Todo { id: 1, text: "Learn Rust".into(), done: true },
        Todo { id: 2, text: "Build Tink".into(), done: false },
        Todo { id: 3, text: "Profit".into(), done: false },
    ]);
    let selected = use_signal(|| 0usize);

    use_input(move |input, key| {
        let len = todos.get().len();
        match input.as_str() {
            "j" => selected.update(|s| *s = (*s + 1).min(len - 1)),
            "k" => selected.update(|s| *s = s.saturating_sub(1)),
            " " => {
                let idx = selected.get();
                todos.update(|t| t[idx].done = !t[idx].done);
            }
            "q" => std::process::exit(0),
            _ => {}
        }
    });

    rsx! {
        Box(flex_direction: FlexDirection::Column, padding: 1) {
            Text(bold: true, color: Color::Cyan) {
                "📝 Todo List"
            }
            Box(margin_top: 1, flex_direction: FlexDirection::Column) {
                for (i, todo) in todos.get().iter().enumerate() {
                    TodoItem(
                        todo: todo.clone(),
                        selected: i == selected.get()
                    )
                }
            }
            Box(margin_top: 1) {
                Text(dim: true) {
                    "j/k: navigate, space: toggle, q: quit"
                }
            }
        }
    }
}

fn todo_item(todo: Todo, selected: bool) -> Element {
    let checkbox = if todo.done { "✓" } else { " " };
    let text_style = if todo.done {
        TextStyle { strikethrough: true, dim: true, ..Default::default() }
    } else {
        TextStyle::default()
    };

    rsx! {
        Box(padding_left: 1) {
            Text(inverse: selected) {
                "[{checkbox}] {todo.text}"
            }
        }
    }
}
```

---

## 七、依赖关系

```toml
# Cargo.toml
[package]
name = "tink"
version = "0.1.0"
edition = "2024"

[dependencies]
# 布局引擎
taffy = "0.7"

# 终端操作
crossterm = "0.28"

# Unicode 宽度计算
unicode-width = "0.2"
unicode-segmentation = "1.10"

# 文本换行
textwrap = "0.16"

# 日志
log = "0.4"

# 内部工具
thiserror = "2.0"

[dependencies.tink-macros]
path = "./tink-macros"

[dev-dependencies]
env_logger = "0.11"
```

```toml
# tink-macros/Cargo.toml
[package]
name = "tink-macros"
version = "0.1.0"
edition = "2024"

[lib]
proc-macro = true

[dependencies]
proc-macro2 = "1.0"
quote = "1.0"
syn = { version = "2.0", features = ["full", "parsing"] }
```

---

## 八、开发路线图

### Phase 1: 核心基础 (MVP)
- [ ] Element 类型定义
- [ ] Box 组件 (Builder)
- [ ] Text 组件 (Builder)
- [ ] Taffy 布局集成
- [ ] Output buffer
- [ ] Terminal 渲染
- [ ] 基础 App 循环

### Phase 2: 状态管理
- [ ] use_signal hook
- [ ] use_effect hook
- [ ] Hook 上下文系统
- [ ] 响应式更新

### Phase 3: 输入处理
- [ ] 按键解析
- [ ] use_input hook
- [ ] 焦点系统
- [ ] use_focus hook

### Phase 4: 宏系统
- [ ] rsx! 宏解析
- [ ] 条件渲染支持
- [ ] 列表渲染支持
- [ ] 组件调用支持

### Phase 5: 高级功能
- [ ] Context 系统
- [ ] Static 组件
- [ ] Transform 组件
- [ ] 边框渲染
- [ ] 滚动支持

### Phase 6: 完善
- [ ] 错误处理
- [ ] 性能优化
- [ ] 文档
- [ ] 示例

---

## 九、与 ink 功能对照检查表

| ink 功能 | Tink 对应 | 状态 |
|----------|-----------|------|
| `<Box>` | `Box` | 🔲 |
| `<Text>` | `Text` | 🔲 |
| `<Newline>` | `Newline` | 🔲 |
| `<Spacer>` | `Spacer` | 🔲 |
| `<Static>` | `Static` | 🔲 |
| `<Transform>` | `Transform` | 🔲 |
| `useState` | `use_signal` | 🔲 |
| `useReducer` | `use_reducer` | 🔲 |
| `useEffect` | `use_effect` | 🔲 |
| `useInput` | `use_input` | 🔲 |
| `useFocus` | `use_focus` | 🔲 |
| `useFocusManager` | `use_focus_manager` | 🔲 |
| `useApp` | `use_app` | 🔲 |
| `useStdin` | `use_stdin` | 🔲 |
| `useStdout` | `use_stdout` | 🔲 |
| `useStderr` | `use_stderr` | 🔲 |
| Flexbox layout | Taffy | 🔲 |
| Text colors | Crossterm | 🔲 |
| Text styles | Crossterm | 🔲 |
| Borders | 自定义实现 | 🔲 |
| Overflow/scroll | 自定义实现 | 🔲 |
| Key parsing | 自定义实现 | 🔲 |
| Screen reader | 🔲 暂不支持 | 🔲 |

---

*文档版本: 1.0*
*最后更新: 2026-01-09*
