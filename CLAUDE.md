GMR 是领域无关的 grounded memory runtime。
架构 SSOT 是 GMR.md。


---

# 一、这十三条是 owner 定的，不要重新论证

```
1   锚定层是一台状态机：δ(state, obs, taken_at, entered_at) → state'
2   探针位置住在 state.position 里，由域设置，不是插件输出
3   position 的形状由域定义；基底只知道去 state.position 取值，不解释里面是什么
4   没有固定状态词表。status 字符串由域定义，基底只拿它做 terminal 比对
5   插件的版本必须是挣来的哈希，哈希的对象是能改变输出的全部输入 —— 不是二进制的字节
6   插件吐状态向量 obs，这是“表示”；锚选择关心哪些方向并写规则，这是“注意力”
7   转换条件的输入只有 state、obs、taken_at、entered_at；基底不提供其他派生量
8   终结态在锚上声明，基底机械兑现不可逆
9   记忆绑在锚上。绑定说“关于什么”，订阅说“什么时候交出来”
10  转换条件写成规则表：守卫 → 完整新状态；第一条匹配即生效
11  state 是可寻址 JSON 结构，不是黑盒；基底能取字段，但不解释字段含义
12  物理三层：基底不产二进制；电池提供可复用实现；域负责插件、锚、装配、CLI
13  表达式语言必须能构造对象，因为每次转换要吐出完整 state，不是 patch
```

# 自举数据不是系统本体

本仓库用 GMR 监督 GMR 自己，所以有一批“使用数据”：

- .anchor/anchors.toml：这个仓库选择锚哪些东西（没有笔记认领的锚才写这里）
- .anchor/probes.toml：这些锚用的探针配方（名字 → 那个名字是什么）
- memories/：人写的记录。frontmatter 声明它关于哪个坐标，锚由此诞生
- .codegraph/：CodeGraph 的本地索引

机器读的声明住在 `.anchor/` 里，人写的记忆留在外面。这不是审美：toml 摊在仓库根目录会被 Agent 当成项目自身的代码读，而记忆藏进点目录就没人看得见 —— 两边都会毁掉这套东西的用处。`.anchor/` 里只有 `anchors.toml` 和 `probes.toml` 跟 git，日志和制品不跟（见 `.anchor/.gitignore`）。

这些文件和 crates/ 里的 GMR 本体不是同一层。不要因为它们在仓库里，就把它们当成系统自带能力、默认规则、产品清单或 crate 依赖。

**`architecture.toml` 不在这一批里。** 它不被任何 GMR 代码读取：不走探针、不走锚、不进日志、不随产品分发。它只是 gate.sh 的依赖禁区清单 —— 一个手写 linter 的配置。之所以不做成锚：一个包有没有依赖 tokio，`cargo tree` 一跑就是确定答案，那是「事实完全决定判断」的一类，明确不该锚。

代码中尽可能的简化注释, 所有声明依靠这套自举的记忆去描述. 必要的注释用英文。

修改 memories/ 或 .anchor/*.toml = 修改本仓库作为 GMR 用户时的判据或记录；通常需要 owner 判断。
修改 crates/ = 修改 GMR 工具本体；必须遵守 crate 边界。
修改 architecture.toml = 修改 gate.sh 的门禁判据，跟 GMR 的语义无关。


# crate 边界：
- gmr-core：词汇 + 内容地址 + Entry + fold。不能知道怎么取事实、怎么算规则、怎么存。
- gmr-expr：纯表达式求值。不能 IO、不能时钟、不能依赖 gmr-core。
  （obs 侧严格 / state 侧宽容、changed() 认 obs↔state 约定：这两条是锚定层的语义决定，不是通用求值器特性——只是恰好没有编译期依赖 gmr-core，不要把"不依赖 gmr-core"读成"不认识锚"。）
- gmr-probe：探针调用契约。不能放具体传输实现。
- gmr-store：存储 trait 和 feature-gated 后端。按可变性切：Journal / BindingStore / Sealer / LinkStore / Queue。
- gmr-runtime：唯一编排层。可以同时看 core / expr / probe / store，但不能替领域做判断。
- gmr：只 re-export。

# 设计原则：
- 当前状态只能来自日志投影。
- 状态词表归领域，基底只兑现 terminal。
- position 在 state.position，基底不解释其结构。
- 转换规则吐完整 state，不是 patch。
- NotFound 是世界答案；ProbeError / Unevaluable 是我们的失败。
- 系统不允许静默失败路径。

# 必须问 owner：
- 要删除真实实现或测试。
- 要改变 crate 边界。
- 要决定某个锚该盯什么方向。
- 要把某种失败路径“不记录”。
- 要修改判据：probe、rules、terminal、state 修订语义。

# Rust 纪律：
- 优先用类型和构造器表达不变量，不靠注释。
- public surface 变更必须说明调用方要知道的新事实。
- core/expr 保持纯；不要引入 IO、数据库、时钟、随机。
- runtime 不写死具体传输、内容提供方、存储后端。
- 新 bug 先写能复现的测试，再修。
- 改完跑相关 cargo test；边界改动跑 gate.sh。

# Rust 工程纪律

- 让类型表达不变量。优先用 newtype、私有字段、校验构造器、Result 返回错误；不要靠注释约束调用方。
- 不要为了省事暴露 public 字段。公开字段等于公开不变量，未来很难收回。
- 能借用就借用。不要用 clone 解决所有权问题；clone 只在所有权确实要分叉、数据很小、或语义上需要快照时使用。
- API 优先接收借用：&T、&str、&[T]。只有函数需要持有数据时才接收 owned 值。
- 返回 owned 值要有理由：新建结果、跨异步边界、存入结构体、或避免悬垂引用。
- 用枚举表达状态和分支，不用字符串、bool 组合、Option 套 Option 表示业务状态。
- 错误要有类型边界。库层返回结构化错误；只有 CLI/render 层把错误变成人类文本。
- 避免无意义 trait。只有存在多个真实 adapter，或者测试/装配需要替换时，才抽 trait。
- 利用零成本抽象：Iterator、match、newtype、泛型可以让接口清楚；不要为了“抽象”引入运行时装箱。
- async 不要扩散。只有真实 IO 边界 async；纯计算、fold、parse、eval 保持同步纯函数。
- 避免全局状态、隐式时钟、随机数。需要时间就作为参数传入，尤其 core/expr 不能自己读时钟。
- 测试跨公开接口写。不要测试私有实现细节；如果必须测私有细节，通常说明模块接口不对。
- 每次改 public surface，都要问：调用方现在必须知道什么新事实？如果答案太多，接口变浅了。
- clone 不是默认解法。先检查能否改成借用、Arc、Cow，或调整数据流让所有权只走一条路。
- 但不要迷信零 clone：日志条目、状态快照、事件记录这些语义上就是快照，该 clone 就 clone。 clone 必须对应语义：快照、共享所有权、或跨生命周期保存。不要用 clone 绕过借用设计。
- core/expr 里的函数应该尽量是纯函数：输入全在参数里，输出全在返回值里。
- runtime 可以拥有 trait object，因为它是装配层；core/expr 不应该为了“灵活”引入 dyn。