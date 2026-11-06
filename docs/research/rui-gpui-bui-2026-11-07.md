# RUI、GPUI、BUI 深入对照与实施决定

2026-11-07。本文支持的决定是 RUI 下一批改动的先后顺序和架构边界，不为三个库打单一总分。无法复现的问题不写成已证 bug；没有同机等价样本时，不写性能胜负。BUI 指 ppmpreetham/BUI，按用户名称与上下文定位。

## 结论

RUI 已有 Entity/View、前后台任务、原生输入协议、Canvas、增量布局/绘制、damage 和 headless 验证。当前最紧急的事情是修复任务上下文与生命周期边界，随后统一安装基线、补原生验收。变高列表和文本撤销是明确应用能力缺口，需要各自独立规格。跨平台与大组件生态是产品投入决定，不能伪装成几个优化 PR。

架构判断：现有 Runtime / Elements / Renderer / Platform 分工可以承接下一阶段；本次发现集中在边界契约，而不是需要推翻分层。保留 FramePipeline 的单一执行路径、复用文字测量和命中坐标。禁止为追赶功能表引入第二套调度器、测量缓存或跨框架适配平台。

## 基线与证据覆盖

| 对象 | 固定版本 | 本轮证据 |
|---|---|---|
| RUI | 1311a1d6b98a601de603d215ebc1d21f408fd7a3 | 固定源码、隔离 core 运行时探针、仓库外真实 Git 依赖编译 |
| GPUI | e231594b6b8818ea8d668ee2eb73e1fb09300654 | 官方源码：List、scheduler、测试调度、平台输入协议 |
| BUI | 610d59d211b4bdf30777471f8dcc8721427c907c | 列表索引/测量/控件/undo/damage/宿主/benchmark 调用链；本机 vlist view 测试 |

三条并行研究分别检查 RUI、GPUI、BUI。主要推断又回到源码和探针核对。此前原生验收记录被明确标为既有记录；本轮没有重新操作三套原生窗口，没有运行 GPUI 全量测试，也没有同机三方 GPU 跑分。BUI 列表测试使用合成数据与测试 painter，不证明真实字体 shaping 或 GPU 表现。GPUI Kit 只作生态背景，不计入 GPUI 核心。

研究时开放 PR：[多窗口 #280](https://github.com/majiayu000/rui/pull/280)、[benchmark 证据 #281](https://github.com/majiayu000/rui/pull/281)、[CI #282](https://github.com/majiayu000/rui/pull/282)、[Space 激活 #283](https://github.com/majiayu000/rui/pull/283)。它们不是基线主线已具备的修复，本轮不重复实现。

## 一、状态与任务：已有骨架，但边界必须成立

### RUI 的真实调用链

`AppContext::spawn` 构造 AsyncApp 和 future → ready queue → FramePipeline 调用 poll_foreground_tasks → TLS 存入当前 AppContext → AsyncApp::update 从 TLS 取得可变引用。后台任务通过 BackgroundRun 和统一 wake 路径进入线程池或 headless inline 队列。[源码](https://github.com/majiayu000/rui/blob/1311a1d6b98a601de603d215ebc1d21f408fd7a3/src/core/executor.rs#L153)、[FramePipeline](https://github.com/majiayu000/rui/blob/1311a1d6b98a601de603d215ebc1d21f408fd7a3/src/core/frame_pipeline.rs#L57)。

不是“没有异步”，而是以下契约失效：

| 发现 | 触发与直接观察 | 最小处理 |
|---|---|---|
| AsyncApp 跨应用错写 | 捕获 A 的 AsyncApp，在 B 的任务中 update。A=10 不变，B 从20变99；无须 panic | 在唯一 update 边界核对创建者的稳定身份，拒绝异主调用 |
| panic 后 TLS 未恢复 | Future::poll panic；宿主 catch_unwind 后继续。观察到 stale_tls=true、poll 外回调仍执行 | 设置 poll context 后用作用域恢复，保留 panic 传播 |
| inline drain 绕过上限 | Future 自唤醒并 Pending；超过100000次 poll 仍不退出，外部3秒超时结束 | 每轮只处理进入时 ready 批次，使用现有 MAX_PARK_ROUNDS |
| 取消 Pending 任务资源延迟释放 | 外部保存 waker；原机制探针 cancel+drain 后未drop，外部wake后才drop | 先完整 runtime 回归确认，再让取消调度清理；不得同步等待长poll |

前三项探针运行于固定源码的隔离副本、`--no-default-features` 定向 lib tests。panic 探针保持 App 存活，没有故意触发悬空指针解引用；没有证明正常 native loop 会 catch-and-continue。跨应用问题指两个 AppContext，不是同一个 App 中的两个窗口。最后一项初始证据是抽取原执行器机制的探针，不能用它冒充完整 runtime 验收；实施阶段需补完整回归。

追踪：[上下文 #284](https://github.com/majiayu000/rui/issues/284)、[调度与取消 #285](https://github.com/majiayu000/rui/issues/285)。

### GPUI 可学什么，不能推出什么

GPUI 任务依赖独立 scheduler，测试可控制 seed、时间和逐步推进；失败 seed 可以重放。RUI 的 inline FIFO 已能稳定重现一种顺序，但不能探索多种交错。应先用已知失效写边界测试，只有确有交错问题才加最小 step/clock 支持，不复制完整 scheduler。

反证也重要：GPUI 的 TestDispatcher::run_until_parked 本身仍是 tick-until-empty，不能断言“照搬 GPUI 就永远不会挂”。drop-cancel 也不等于能抢占正在运行的同步 CPU 代码。[GPUI 测试入口](https://github.com/zed-industries/zed/blob/e231594b6b8818ea8d668ee2eb73e1fb09300654/crates/gpui/src/test.rs)、[scheduler](https://github.com/zed-industries/zed/blob/e231594b6b8818ea8d668ee2eb73e1fb09300654/crates/scheduler/src/executor.rs)、[测试调度](https://github.com/zed-industries/zed/blob/e231594b6b8818ea8d668ee2eb73e1fb09300654/crates/scheduler/src/test_scheduler.rs)。

## 二、列表：优化阅读行为，不只计算能画多少行

RUI 等高 VirtualList 已真正只构建可见范围及 overscan，保留重叠行，有 scroll_to_item、滚动条和裁剪。DataList 的统一 row_height 不解决变高需求。[实现](https://github.com/majiayu000/rui/blob/1311a1d6b98a601de603d215ebc1d21f408fd7a3/src/elements/virtual_list.rs#L103)、[测试](https://github.com/majiayu000/rui/blob/1311a1d6b98a601de603d215ebc1d21f408fd7a3/tests/virtual_list.rs)。

GPUI List 采用 item index + 行内 offset，并在 splice 时调整锚点。屏外内容变高需要显式通知，例如 remeasure_items；结构变化用 splice/reset。它不是自动感知任意数据重排。其 remeasure 区分局部高度变化和整体尺寸变化；还会额外保留焦点项以支持交互。[List](https://github.com/zed-industries/zed/blob/e231594b6b8818ea8d668ee2eb73e1fb09300654/crates/gpui/src/elements/list.rs)。

BUI 使用分块高度索引和 Fenwick 树定位，未测块先保存估计；布局固定锚点，再测量可见项并修正高度。prepend/remove 是显式索引编辑通知，不是按业务 key 自动追踪。Bottom 是独立跟随状态，用户上滚后停止跟随。[索引](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/bindings/rust/blenderui-vlist/src/index/mod.rs)、[帧布局](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/bindings/rust/blenderui-vlist/src/view/frame.rs)、[编辑](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/bindings/rust/blenderui-vlist/src/view/edit.rs)。

本机已观察测试断言通过：前插历史不跳动、上方行变高不移动锚点、缩窄窗口保留锚点、用户上滚后不被新消息拉回底部。它们比“支持千万行”更适合作为 RUI 验收。[测试](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/bindings/rust/blenderui-vlist/tests/view.rs)。

### BUI 实现的实际代价

- 定位为树级查找加块内查找，但跨块插入会重建树；删除整块时可能重复重建，不能把所有编辑写成 O(log N)。
- 校准估高与 trim 会遍历或重建块索引；不是每一条路径都只访问 viewport。
- 文字缓存键含文本、宽度、字号，字体/主题变更依赖显式清理。缓存不是自动感知全部字体属性。
- 零分配测试只覆盖预热后的小幅往返滚动，不覆盖冷启动、大跳转或 trim。

依据：[索引编辑和回收](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/bindings/rust/blenderui-vlist/src/index/mod.rs#L314)、[文字测量](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/bindings/rust/blenderui-vlist/src/measure.rs)、[缓存](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/bindings/rust/blenderui-paint/src/text_cache.rs)。

RUI 的布局约束是关键：Element::layout 阶段主要构建节点；现有 compute_layout 消耗 LayoutContext 并结束 retained pass。不能在每一行内部直接调用它，更不能用上一帧宽度代替本帧约束。[LayoutContext](https://github.com/majiayu000/rui/blob/1311a1d6b98a601de603d215ebc1d21f408fd7a3/src/elements/element/layout.rs)。

决定：保留等高快路径，给变高列表单独规格。初版固定 viewport 约束，只增加必要的内部子树测量入口，复用原文字缓存与布局回收；不用 PaintSink、远程分页、估计器插件或 skeleton 平台。详见[变高列表规格](../specs/variable-height-list.md)。规格存在不表示实现完成。

## 三、输入、焦点与撤销

RUI 已有 NSTextInputClient、UTF-16 范围、选区几何、composition owner 和失焦清理；GPUI 的 marked/replacement range、selected range、bounds 等接口是可对照的契约，不是 RUI 从零缺失的模块。[RUI 输入](https://github.com/majiayu000/rui/blob/1311a1d6b98a601de603d215ebc1d21f408fd7a3/src/platform/mac/text_input.rs)、[组合态](https://github.com/majiayu000/rui/blob/1311a1d6b98a601de603d215ebc1d21f408fd7a3/src/platform/mac/ime_state.rs)、[GPUI 平台输入协议](https://github.com/zed-industries/zed/blob/e231594b6b8818ea8d668ee2eb73e1fb09300654/crates/gpui/src/platform.rs)。

既有 RUI 原生记录验证了 Tab/ShiftTab、禁用/隐藏跳过、AX 焦点、Unicode 粘贴与保存；没有验证真实中文候选选择或 VoiceOver 朗读。不要把粘贴 Unicode 当成 IME，也不要把 AX 树可读当成语音可用。[验收记录](https://github.com/majiayu000/rui/blob/1311a1d6b98a601de603d215ebc1d21f408fd7a3/docs/native-interaction-acceptance.md)。

BUI 明确没有平台无障碍和 IME，RTL 有重排但未完整 shaping，macOS 仍 CPU 绘制。因此它适合参考控件行为，不是输入法/无障碍成熟度的统一标准。[限制](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/README.md#40-status-and-roadmap)。

### 借鉴操作规则，而不是代码形状

BUI 数字控件区分按下、超过阈值的拖动、精细调整、确认和取消；文本又区分 live updates 与确认后提交。普通文本 Escape 和 live text/HTML range Escape 不同，不能设计一个“所有控件 Escape 都回滚”的硬规则。

它有编辑框局部撤销与跨控件事务两层；typing 合并按 interaction 计数，不是时间窗口。RUI 不必照搬这两层或常量。已查 RUI TextBuffer/命令、Input、TextArea、action 模块尚无 Undo/Redo 历史实现；最小补项是单输入状态拥有文本与选区历史，IME 一次提交一个事务。详见[文本撤销规格](../specs/text-undo.md)。[BUI 输入状态机](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/kit/intern/kit_widgets.c#L1973)、[撤销](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/kit/intern/kit_core.c#L3458)、[RUI 文本编辑](https://github.com/majiayu000/rui/blob/1311a1d6b98a601de603d215ebc1d21f408fd7a3/src/core/text_editing/buffer.rs)。

## 四、绘制、空闲和 benchmark 的含义

RUI 已有随机编辑 damage/full 像素对照、未变化不重画、移动旧新区域、target 漏帧和滚动恢复测试。不能将“引入 damage”列为新增工作。[回归测试](https://github.com/majiayu000/rui/blob/1311a1d6b98a601de603d215ebc1d21f408fd7a3/tests/damage_frames.rs)。

BUI 先合并 dirty rectangles，layout dirty 可升级完整重画；无 dirty 时返回，但图像/动画处理仍可能执行。Cocoa 宿主空闲时进入等待。源码支持事件驱动设计，不证明进程 CPU 恒为零。[渲染路径](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/kit/intern/kit_core.c#L2194)、[宿主](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/kit/intern/kit_window_cocoa.m#L1690)。

### 为什么本轮不宣布 BUI 或 RUI 更快

1. BUI benchmark 的 frame() 在 render 回调入口；first_frame 等待计数大于0。它标记开始构建，不是 GPU 完成或屏幕呈现；FPS 也首先是 render 回调次数。[BUI 入口](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/bindings/rust/bench/bui-bench/src/main.rs#L15)、[GPUI 入口](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/bindings/rust/bench/gpui-bench/src/scene.rs#L19)、[计量](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/bindings/rust/bench/common/measure.rs#L159)。
2. one 场景仍每帧重建整棵 scene，只改一个格子的值；不能替代 retained-view 最小通知路径的比较。
3. 历史 Windows Ryzen 跑分不能迁移成 macOS Metal 排名，且所用 gpui 0.2.2 与本轮最新源码不同。
4. benchmark README 通用说明三次中位数，但当前结果节标 reps1；不同批次不能混称三次统计。[协议与结果](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/bindings/rust/bench/README.md)。
5. RUI/GPUI 自己的采样口径仍由 [#277](https://github.com/majiayu000/rui/issues/277) 处理，本轮不把旧 GPU 倍数当优化依据。

下一次性能实验协议：同机串行、固定提交/release build、同视口/DPI/字体/文本与帧率策略；分别测冷启动、静止、单行修改、滚动、resize。记录 build/layout/paint、GPU完成、present 与CPU唤醒各自指标，先验证截图/输出等价。每格至少三轮预热后的样本；若轮间波动大，不宣布排名。新旧 RUI 比较先于跨框架排名。没有真实 GPU 完成信号的项目只报告CPU阶段。

## 五、发布、组件和架构投入

README pin 99d08da9，而 getting-started pin dbffe70。更具体的是旧 README pin 的树尚无当前 API 文档展示的 executor/prompt/menu 模块。依赖、文档和例子应有共同基线，不能靠“仓库例子能编译”代替用户安装体验。[README](https://github.com/majiayu000/rui/blob/1311a1d6b98a601de603d215ebc1d21f408fd7a3/README.md#L45)、[File Dialogs API](https://github.com/majiayu000/rui/blob/1311a1d6b98a601de603d215ebc1d21f408fd7a3/docs/API.md#L151)。

本轮把两份完整文档程序提取到仓库外项目，使用实际 Git 依赖 1311a1d，两个 binary 均完成 cargo check。此证据只证明构建，不证明真实窗口交互。最小实施规格：[安装入口 #286](https://github.com/majiayu000/rui/issues/286)：两份入口统一到这个已验证 revision，保留 macOS 限制、同名 crate 警告与“非新版本发布”说明。不发 crate、不改包名、不建安装器。

GPUI 有多个桌面平台后端；BUI 也有 Windows/Linux 路径，但 macOS 实验性。RUI 目前承诺 macOS 是合理边界。只有用户真实需要 Windows/Linux 时才投入平台实现；不要先在不支持的平台 CI 上制造红灯。GPUI Kit 的应用组件目录是生态价值，不等于 GPUI 核心全包。[GPUI 平台说明](https://github.com/zed-industries/zed/blob/e231594b6b8818ea8d668ee2eb73e1fb09300654/crates/gpui/README.md)。

组件扩充应由一个实际使用的编辑器/设置工具驱动：输入/保存、异步失败、撤销、空状态和退出。BUI 的 HTML/Qt/tkinter/多语言兼容层不适合当前 RUI；GPL 仓库实现本轮只读，行为独立实现，不复制代码。[BUI 架构说明](https://github.com/ppmpreetham/BUI/blob/610d59d211b4bdf30777471f8dcc8721427c907c/README.md#15-how-it-works)。

## 实施台账与验收决定

| 工作 | 处理 | 验收边界 |
|---|---|---|
| #284 上下文归属与panic恢复 | 本批实施 | 异主拒绝、unwind恢复、后续合法task仍可运行 |
| #285 inline进度与取消释放 | 本批实施 | 自唤醒触达现有上限，取消无需外部wake释放资源 |
| #286 入门基线 | 本批实施 | 两份文档的外部消费者构建一致 |
| [变高列表 #287](https://github.com/majiayu000/rui/issues/287) | 独立 Issue + Spec，待实现 | 测量契约、稳定锚点、命中与retained/fresh一致 |
| [文本撤销 #288](https://github.com/majiayu000/rui/issues/288) | 独立 Issue + Spec，待实现 | 文本/选区、IME事务、redo分支、控件隔离 |
| #280–#283 | 已有PR继续验收 | 不把开放PR计入主线，不重复建设 |
| #177 shader预编译、#191调度优先级 | 已有Issue按profiling推进 | 有收益证据才实现，不从竞品宣传反推瓶颈 |
| 真机IME/VoiceOver、跨平台 | 独立后续验收/投入 | 当前没有测试结论，不宣称产品成熟 |

本轮选定可复现运行时缺陷与安装入口形成实现 PR；更大能力留下可审查规格，不塞入同一个修复。采用现有架构上的边界修正，否定无证据重写。报告、规格、实现和真实应用验收是四个不同状态。

## 本批交付与实现验收

- [PR #289](https://github.com/majiayu000/rui/pull/289)：本报告、两份能力规格和安装基线修正；关闭 #286，#287/#288 仍为规格待实现。
- [PR #290](https://github.com/majiayu000/rui/pull/290)：实现 #284/#285，包含[任务边界 Spec](https://github.com/majiayu000/rui/blob/29f6eca2d121ae80e1e839c4c1a8e50fb5946eda/docs/specs/runtime-task-boundaries.md)与[边界回归测试](https://github.com/majiayu000/rui/blob/29f6eca2d121ae80e1e839c4c1a8e50fb5946eda/src/core/executor_boundaries_tests.rs)。

四个基线回归先失败后通过。完整 runtime 已确认取消且外部 waker 保留时，推进执行器会释放 future；不再只依靠原机制片段。默认特性下重新运行 executor 和开启 RUI_VERIFY_REPLAY 的 spawn 集成测试：异主回调不执行且 A/B 保持10/20、应用移动后合法update仍成功、捕获panic后任务外update被拒绝、后续合法任务写入7、自唤醒触发原有10000轮诊断、线程中正在poll时cancel不阻塞、后台接力最终显示 step 9。

已验：这些行为对应的断言与实际测试输出、两个外部消费者编译、独立代码和文档审查。没验：Miri、穷尽并发交错、原生GUI/IME/VoiceOver及三方性能；GitHub完整CI状态以PR页面为准。相邻情况：嵌套update、detach、首次poll前取消、FIFO和真实后台线程均覆盖。取消不能抢占一个永不返回的同步poll；panic恢复不承诺自动重试原来未完成的任务。

这些提交处于PR阶段，本报告不表示已合并、发布或完成真实用户验收。
