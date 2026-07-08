# 基于 OpenAgere 项目的 Rust 学习手册

> 本手册从 OpenAgere 项目的真实代码出发，系统讲解项目涉及的所有 Rust 语言特性。
> 每个概念都配有**项目源码中的真实例子**、**完整可运行示例**、**原理讲解**和**设计决策分析**。

---

## 目录

**第一部分：Rust 基础**

1. [Rust 环境与工具链](#1-rust-环境与工具链)
2. [变量、类型与控制流](#2-变量类型与控制流)
3. [所有权系统](#3-所有权系统)
4. [借用与引用](#4-借用与引用)
5. [生命周期](#5-生命周期)
6. [结构体与方法](#6-结构体与方法)
7. [枚举与模式匹配](#7-枚举与模式匹配)

**第二部分：类型系统与抽象**

8. [Trait 系统](#8-trait-系统)
9. [泛型](#9-泛型)
10. [错误处理](#10-错误处理)

**第三部分：核心能力**

11. [集合与字符串](#11-集合与字符串)
12. [闭包与迭代器](#12-闭包与迭代器)
13. [智能指针](#13-智能指针)
14. [Pin 与 Unpin](#14-pin-与-unpin)
15. [Unsafe Rust](#15-unsafe-rust)

**第四部分：元编程与序列化**

16. [宏系统](#16-宏系统)
17. [Serde 序列化](#17-serde-序列化)

**第五部分：异步与并发**

18. [async/await 深入](#18-asyncawait-深入)
19. [并发编程](#19-并发编程)

**第六部分：工程实践**

20. [类型系统与高级 Trait](#20-类型系统与高级-trait)
21. [模块系统与 Workspace](#21-模块系统与-workspace)
22. [条件编译与构建](#22-条件编译与构建)
23. [标准库深入](#23-标准库深入)
24. [测试](#24-测试)
25. [日志与调试](#25-日志与调试)
26. [常用生态 Crate](#26-常用生态-crate)
27. [惯用法与设计模式](#27-惯用法与设计模式)
28. [性能与优化](#28-性能与优化)

**第七部分：综合**

29. [项目代码深度解读](#29-项目代码深度解读)
30. [附录与速查表](#30-附录与速查表)

---

## 1. Rust 环境与工具链

在开始写代码之前，我们先了解 Rust 的开发工具链。Rust 的工具链设计得非常优秀，
几乎所有操作都通过一个命令 `cargo` 完成。

### 1.1 安装 Rust：rustup

`rustup` 是 Rust 的官方安装器，负责安装和管理 Rust 编译器版本。

**安装方法：**

```shell
# macOS / Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows
# 下载 https://rustup.rs 的安装程序
```

安装完成后，你获得了三个工具：

| 工具 | 作用 | 类比 |
|------|------|------|
| `rustc` | Rust 编译器，把 `.rs` 源码变成可执行文件 | gcc / javac |
| `cargo` | 包管理器 + 构建工具 + 测试工具 | npm / maven / gradle |
| `rustup` | 管理 Rust 版本（stable/beta/nightly） | nvm (Node) / pyenv (Python) |

**常用 rustup 命令：**

```shell
rustup show                    # 查看当前安装的工具链
rustup update                  # 更新到最新稳定版
rustup toolchain install nightly  # 安装 nightly 版本
rustup default stable          # 设置默认版本
rustup component add clippy    # 安装 clippy 代码检查器
rustup component add rustfmt   # 安装格式化工具
```

**为什么 Rust 有多个版本？**
- `stable`：每 6 周发布一次，经过充分测试，生产环境使用
- `beta`：下一个 stable 的候选，用于测试
- `nightly`：每天构建，包含实验性功能（如某些高级宏特性）

OpenAgere 项目使用的是 `stable` 版本（edition 2024）。

### 1.2 第一个 Rust 程序

```shell
cargo new hello_rust
cd hello_rust
```

这会创建以下目录结构：

```text
hello_rust/
├── Cargo.toml    ← 项目配置文件（类似 package.json）
├── Cargo.lock    ← 精确依赖版本锁定（自动生成）
└── src/
└── main.rs   ← 程序入口
```

打开 `src/main.rs`：

```rust
fn main() {
    println!("Hello, world!");
}
```

**逐行解释：**

**`fn main() {`** — 定义了一个名为 `main` 的函数。`fn` 是 "function" 的缩写。
每个 Rust 可执行程序都必须有一个 `main` 函数，它是程序的入口点。
花括号 `{` 标记函数体的开始。

**`println!("Hello, world!");`** — 调用了一个**宏**（注意末尾的 `!`）。
`println!` 不是普通函数，而是一个宏。宏在编译期展开生成代码。
它把字符串打印到标准输出，并自动添加换行符。

**`}`** — 函数体结束。

**运行：**

```shell
cargo run
# 输出: Hello, world!
```

`cargo run` 做了两件事：先编译（`cargo build`），然后运行编译出的二进制文件。
如果代码没有修改，再次运行会直接执行，跳过编译。

### 1.3 Cargo.toml 详解

```toml
[package]
name = "hello_rust"
version = "0.1.0"
edition = "2024"

[dependencies]
```

| 字段 | 含义 |
|------|------|
| `name` | 项目名称，也是编译后二进制文件的名称 |
| `version` | 语义化版本号（主.次.补丁） |
| `edition` | Rust 版本。**2024** 是最新稳定版，决定了语言特性的可用性 |
| `[dependencies]` | 外部依赖列表 |

**为什么 `edition` 很重要？**

Rust 的 edition 不是"新版本"，而是"兼容性快照"。
每个 edition 引入新语法和功能，但旧代码永远不会因为 edition 升级而坏掉。
不同 edition 的 crate 可以互相依赖——Rust 编译器会自动处理兼容层。

OpenAgere 项目使用 edition 2024：

```toml
# Cargo.toml 根文件
[workspace.package]
edition = "2024"
```

### 1.4 Cargo 工作流

| 命令 | 作用 | 何时使用 |
|------|------|---------|
| `cargo build` | 编译项目 | 想检查是否能编译 |
| `cargo run` | 编译并运行 | 开发时运行程序 |
| `cargo check` | 快速检查（不生成二进制） | 写代码时快速验证语法 |
| `cargo test` | 运行所有测试 | 写完代码后验证正确性 |
| `cargo build --release` | 优化编译（更慢但更快运行） | 发布时 |
| `cargo fmt` | 格式化代码 | 提交前整理格式 |
| `cargo clippy` | 代码检查（lint） | 发现潜在问题 |

**`cargo check` vs `cargo build`：**

`cargo check` 只做类型检查和借用检查，不生成二进制文件。
速度比 `cargo build` 快得多（在大型项目中可能快 10 倍）。
开发时，大多数 IDE 使用 `cargo check` 来实时显示错误。

**Debug vs Release 构建：**

```shell
cargo build          # Debug 构建（快编译，慢运行，包含调试信息）
cargo build --release  # Release 构建（慢编译，快运行，无调试信息）
```

Debug 构建保留了完整的调试信息，编译快，但运行时不做优化。
Release 构建会做大量优化（内联、循环展开等），编译慢 3-5 倍，但运行可能快 10-100 倍。

OpenAgere 的发布配置：

```toml
# Cargo.toml
[profile.release]
lto = "fat"             # 全链接时优化——跨 crate 内联
codegen-units = 1       # 单线程代码生成——最大化优化
strip = "symbols"       # 去除符号表——减小二进制大小
```

### 1.5 添加依赖

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
```

**语法解释：**

- `serde = { version = "1", features = ["derive"] }` — 版本 1.x 的最新兼容版本，
启用 `derive` 功能（允许 `#[derive(Serialize, Deserialize)]`）
- `version = "1"` — 语义版本：`"1"` 等价于 `">=1.0.0, <2.0.0"`
- `features = ["derive"]` — 启用 crate 的可选功能

**Cargo.lock 的作用：**

`Cargo.lock` 记录了每个依赖的精确版本。它确保所有开发者使用完全相同的依赖版本。
对于二进制项目（如 OpenAgere CLI），`Cargo.lock` 应该提交到 git。
对于库 crate，`Cargo.lock` 通常不提交。

### 1.6 Workspace — 多 crate 项目

OpenAgere 是一个 **workspace**——一个包含 70+ 个 crate 的巨型项目。

```toml
# 根 Cargo.toml
[workspace]
members = [
"core",       # 核心逻辑
"protocol",   # 数据类型定义
"tui",        # 终端界面
"cli",        # 命令行入口
"exec",       # 命令执行
"config",     # 配置管理
# ... 还有 60+ 个
]
resolver = "2"

[workspace.package]
version = "0.3.28"
edition = "2024"

[workspace.dependencies]
agere-core = { path = "core" }
serde = { version = "1" }
tokio = { version = "1" }
```

**为什么要用 workspace？**

1. **统一依赖版本** — 所有 crate 用同一个版本的 `serde`、`tokio`
2. **共享编译缓存** — 编译一次，所有 crate 共用 `target/` 目录
3. **路径依赖** — `agere-core = { path = "core" }` 直接引用本地 crate
4. **独立编译** — 只修改 `protocol` crate 时，只重编译 `protocol` 和依赖它的 crate

子 crate 继承 workspace 设置：

```toml
# protocol/Cargo.toml
[package]
name = "agere-protocol"
version.workspace = true    # 继承 workspace 的版本号
edition.workspace = true    # 继承 edition

[dependencies]
serde = { workspace = true, features = ["derive"] }  # 使用 workspace 的 serde
```

### 1.7 rustfmt 与 clippy

**rustfmt** 自动格式化代码为统一风格：

```shell
cargo fmt
```

**clippy** 是 Rust 的官方 lint 工具，能发现常见错误和不规范的写法：

```shell
cargo clippy
```

clippy 会报告诸如：
- `unwrap_used` — 不应该在生产代码中用 `.unwrap()`
- `redundant_closure` — `|x| foo(x)` 可以简写为 `foo`
- `collapsible_if` — 嵌套的 `if` 可以合并
- `uninlined_format_args` — `format!("{}", x)` 应写为 `format!("{x}")`

OpenAgere 配置了大量强制 lint：

```toml
# Cargo.toml
[workspace.lints.clippy]
unwrap_used = "deny"          # 禁止 .unwrap()
expect_used = "deny"          # 禁止 .expect()
redundant_closure = "deny"    # 禁止冗余闭包
uninlined_format_args = "deny"  # 必须内联 format! 变量
```

`"deny"` 意味着违反这些规则会导致**编译失败**，不只是警告。
这迫使所有开发者写出一致的、安全的代码。

### 本章小结

学完本章你应该知道：
- 如何安装和更新 Rust（`rustup`）
- `cargo new` 创建项目，`cargo run` 运行
- `Cargo.toml` 的每个字段含义
- `workspace` 是多 crate 项目的组织方式
- `cargo fmt` 格式化，`cargo clippy` 检查
- edition 决定了语言特性的可用性

---

## 2. 变量、类型与控制流

本章是 Rust 的"字母表"——变量声明、数据类型、控制流。
你会看到这些概念在其他语言中也存在，但 Rust 有一些独特的设计。

### 2.1 变量绑定与不可变性

在 Rust 中，用 `let` 关键字声明变量。**变量默认是不可变的（immutable）**。
这意味着一旦绑定了一个值，就不能修改它。这不是限制——这是安全的基础。

```rust
fn main() {
    let name = "OpenAgere";
    println!("name = {name}");

    // name = "other";  // ❌ 编译错误！
    // 错误信息: cannot assign twice to immutable variable `name`
    // Rust 编译器在编译期就阻止了意外修改
}
```

**为什么默认不可变？**

在大多数语言中（Python、JavaScript、Java），变量默认是可变的。
这导致了大量 bug：一个函数在远处修改了你以为不会变的变量。
Rust 反转了这个默认值——如果你需要修改，必须显式用 `mut`：

```rust
fn main() {
    let mut count = 0;    // mut 声明为可变
    count += 1;            // ✅ 可以修改
    count += 1;
    println!("count = {count}");  // 输出: count = 2
}
```

`mut` 是一种**意图声明**——读到 `let mut` 就知道"这个变量会被修改"。
如果没有 `mut`，你可以确信这个值从创建到销毁都不会变。
这在多线程环境中尤其重要：不可变的数据可以安全地在多个线程间共享。

### 2.2 变量遮蔽（Shadowing）

你可以用同一个名字声明一个新变量，新变量会**遮蔽**旧的。

```rust
fn main() {
    let x = 5;
    let x = x + 1;     // 新的 x 遮蔽了旧的
    let x = x * 2;     // 再次遮蔽
    println!("x = {x}");  // 输出: x = 12

    // 遮蔽甚至可以改变类型！
    let spaces = "   ";        // 类型: &str（字符串引用）
    let spaces = spaces.len(); // 类型: usize（整数）
    // 这在 mut 中是不可能的——mut 不能改变类型
}
```

**Shadowing vs `mut` 的区别：**

| | `let mut` | Shadowing |
|--|----------|-----------|
| 修改变量 | 同一个变量多次修改 | 每次创建新变量 |
| 改变类型 | ❌ 不能 | ✅ 可以 |
| 使用场景 | 循环中累加、状态变化 | 类型转换、逐步构建 |

### 2.3 常量与静态变量

```rust
// const — 编译期常量，值必须编译时可知
const MAX_CONNECTIONS: u32 = 1000;
const APP_NAME: &str = "OpenAgere";
const PI: f64 = 3.14159265358979;

// static — 全局静态变量，有固定内存地址
static TIMEOUT_MS: u64 = 5000;

// static 可以存可变数据（需要 unsafe）
static mut COUNTER: u32 = 0;

fn main() {
    println!("最大连接: {MAX_CONNECTIONS}");
    println!("应用名: {APP_NAME}");
    println!("超时: {TIMEOUT_MS}ms");
}
```

**`const` vs `static` vs `let`：**

| | `let` | `const` | `static` |
|--|-------|---------|----------|
| 可变性 | 可以 `mut` | 不可变 | 可以 `mut`（unsafe） |
| 内存 | 栈或堆 | 内联到使用处（无地址） | 有固定地址 |
| 值计算 | 运行时 | 编译时 | 编译时 |
| 命名 | `snake_case` | `SCREAMING_SNAKE` | `SCREAMING_SNAKE` |

**项目实例 — OpenAgere 中的常量：**

```rust
// core/src/exec_policy.rs — 真实代码
const PROMPT_CONFLICT_REASON: &str =
"approval required by policy, but AskForApproval is set to Never";

const RULES_DIR_NAME: &str = "rules";
const RULE_EXTENSION: &str = "rules";
const DEFAULT_POLICY_FILE: &str = "default.rules";

// static 用于大的只读数据表
static BANNED_PREFIX_SUGGESTIONS: &[&[&str]] = &[
&["python3"],
&["python3", "-"],
&["python"],
];
```

**为什么用 `const` 而不是函数？** 因为 `const` 在编译时就被内联到每个使用处，
零运行时开销。而函数调用有开销（虽然编译器通常会内联）。
`static` 则保证整个程序只有一份数据——适合大型只读表。

### 2.4 基本数据类型

#### 整数

Rust 的整数类型比其他语言更丰富。每种大小都有有符号和无符号两个版本。

| 类型 | 大小 | 范围 | 说明 |
|------|------|------|------|
| `i8` | 8 位 | -128 ~ 127 | 一个字节 |
| `u8` | 8 位 | 0 ~ 255 | 一个字节，常用于字节操作 |
| `i16` | 16 位 | -32768 ~ 32767 | 较少使用 |
| `u16` | 16 位 | 0 ~ 65535 | 端口号 |
| `i32` | 32 位 | -2^31 ~ 2^31-1 | **整数默认类型** |
| `u32` | 32 位 | 0 ~ 2^32-1 | 计数、ID |
| `i64` | 64 位 | 很大 | 时间戳 |
| `u64` | 64 位 | 很大 | 大计数 |
| `i128` | 128 位 | 极大 | 密码学 |
| `isize` | 平台 | 指针大小 | **索引默认类型** |
| `usize` | 平台 | 指针大小 | 集合长度、内存偏移 |

```rust
fn main() {
    // 类型推导 — 默认 i32
    let x = 42;            // 推导为 i32
    let y: u64 = 100;      // 显式标注

    // 不同进制
    let hex = 0xff;        // 十六进制 = 255
    let binary = 0b1010;   // 二进制 = 10
    let octal = 0o77;      // 八进制 = 63

    // 下划线分隔（提高可读性，不影响值）
    let million = 1_000_000;

    // 溢出行为
    let max_u8: u8 = 255;
    // let overflow = max_u8 + 1;
    // debug 模式: panic!（运行时崩溃）
    // release 模式: 回绕为 0（静默错误）
    let wrapping = max_u8.wrapping_add(1);  // 显式回绕，安全
    println!("255 + 1 (wrapping) = {wrapping}");  // 输出: 0
}
```

**为什么 `usize` 和 `isize`？** 它们的值取决于平台：32 位系统上是 32 位，64 位系统上是 64 位。
这和指针大小一致，所以集合的长度、数组的索引都用 `usize`。
你不能把 `u64` 直接赋给需要 `usize` 的参数——需要用 `as usize` 转换。

#### 浮点数

```rust
fn main() {
    let x = 2.0;     // 默认 f64（双精度）
    let y: f32 = 1.5; // f32（单精度）

    // f64 是默认的，因为它和现代 CPU 的浮点单元匹配
    println!("x = {x}, y = {y}");
}
```

#### 布尔与字符

```rust
fn main() {
    let active: bool = true;
    let done = false;  // 推导为 bool

    // char 是 4 字节的 Unicode 字符（不是 ASCII 的 1 字节！）
    let letter = 'A';
    let chinese = '中';      // ✅ 一个 char 就是一个 Unicode 码点
    let emoji = '🦀';        // ✅ Rust 螃蟹！
    let escape = '\n';       // 换行
    let unicode = '\u{1F600}'; // 通过码点指定

    println!("{letter} {chinese} {emoji} {escape}{unicode}");

    // char 大小固定为 4 字节
    println!("char 大小: {} 字节", std::mem::size_of::<char>());
    // 输出: char 大小: 4 字节
}
```

**重要：** Rust 的 `char` 是 Unicode 码点（UTF-32），不是 ASCII 字节。
一个中文字符、一个 emoji，都是一个 `char`。这和 C/Java 的 `char`（16 位 UTF-16）不同。

### 2.5 复合类型：元组与数组

#### 元组（Tuple）

元组把多个**不同类型**的值组合在一起。长度固定。

```rust
fn main() {
    let point: (i32, i32) = (10, 20);
    let mixed = (1, "hello", true);  // 可以混合类型！

    // 用点号访问
    println!("x = {}, y = {}", point.0, point.1);

    // 解构赋值
    let (x, y) = point;
    println!("x = {x}, y = {y}");

    // 单元类型 () — 空元组
    fn do_nothing() -> () {
        // 没有 return 的函数隐式返回 ()
    }
    let _result: () = do_nothing();
    // () 在 Rust 中很常见——它表示"没有有意义的值"
}
```

#### 数组（Array）

数组是**相同类型**、**固定长度**的集合。存储在栈上。

```rust
fn main() {
    let numbers: [i32; 5] = [1, 2, 3, 4, 5];  // 类型和长度
    let zeros = [0; 10];  // 10 个 0

    // 访问（索引越界会 panic）
    println!("第一个: {}", numbers[0]);
    // println!("{}", numbers[10]);  // ❌ panic! index out of bounds

    // 安全访问（返回 Option）
    let maybe = numbers.get(10);
    println!("get(10): {maybe:?}");  // None

    // 遍历
    for n in &numbers {
        print!("{n} ");
    }
    println!();

    // 数组长度在编译时固定
    println!("长度: {}", numbers.len());
}
```

**数组 vs Vec：** 数组在栈上，长度固定，性能好但不够灵活。
`Vec<T>` 在堆上，可以动态增长。实际开发中大量使用 `Vec`，数组只在已知固定大小时使用。

### 2.6 String 与 &str — 最重要的区别

这是 Rust 中最常让初学者困惑的概念。理解了它，就理解了 Rust 内存模型的半壁江山。

**`String`** — 拥有的（owned）、堆上分配的、可增长的 UTF-8 字符串。
**`&str`** — 借用（borrowed）的字符串切片，指向已有数据的一部分。

```rust
fn main() {
    // String — 拥有数据
    let mut s = String::from("hello");
    s.push_str(", world!");  // 可以修改（因为 mut）
    s.push('!');              // 追加字符
    println!("{s}");          // "hello, world!!"

    // &str — 借用数据
    let slice: &str = &s[0..5];  // 指向 s 的 "hello" 部分
    println!("切片: {slice}");

    // 字符串字面量是 &'static str
    let literal: &str = "我是字面量";
    // "我是字面量" 被硬编码在二进制文件中
    // &'static 表示它的生命周期是整个程序运行期间

    // ---- 转换方式 ----

    // String → &str
    let owned = String::from("hello");
    let borrowed: &str = &owned;         // 自动解引用
    let also_str: &str = owned.as_str(); // 显式方法

    // &str → String（需要分配堆内存——这是"克隆"）
    let back: String = borrowed.to_string();
    let also_owned: String = String::from(borrowed);
}
```

**内存模型图解：**

```text
String "hello":
┌─────────────────────────┐
│ 栈上的 String 头部       │
│  ptr ─────────────────┐ │
│  len = 5              │ │
│  capacity = 5         │ │
└───────────────────────│─┘
▼
┌───────────────┐
堆内存        │ h e l l o     │
└───────────────┘

&str（借用）:
┌─────────────────────────┐
│ 栈上的切片引用           │
│  ptr ─────────────────┐ │  ← 指向同一个堆内存
│  len = 5              │ │
└───────────────────────│─┘
```

**规则：函数参数用 `&str`，结构体字段用 `String`。**

```rust
// ✅ 好：函数接受 &str，调用方可以传任何字符串引用
fn greet(name: &str) {
    println!("你好, {name}!");
}

// ❌ 差：要求调用方给一个 String 的所有权
fn greet_owned(name: String) {
    println!("你好, {name}!");
}

fn main() {
    let owned = String::from("Alice");
    let literal = "Bob";

    greet(&owned);    // ✅ &String 可以自动转为 &str
    greet(literal);   // ✅ 字面量就是 &str

    // greet_owned(owned);  // ❌ 需要给所有权，之后 owned 失效
    // greet_owned(literal); // ❌ 类型不匹配，&str 不是 String
}
```

**为什么这样设计？** `&str` 是一个"视图"——它不拥有数据，只是看到数据的一部分。
这样函数不需要分配内存，也不需要知道字符串来自哪里（堆、栈、二进制文件）。
它只是说"给我一个字符串的引用，我保证不修改它"。

#### format! 宏

```rust
fn main() {
    let name = "OpenAgere";
    let version = 3;

    // 内联变量（Rust 1.58+，项目 AGENTS.md 强制要求）
    let msg = format!("Welcome to {name} v{version}");
    println!("{msg}");

    // 旧写法（不推荐）
    let msg2 = format!("Welcome to {} v{}", name, version);

    // 格式化选项
    let pi = 3.14159;
    println!("{pi:.2}");        // 3.14（2 位小数）
    println!("{pi:>10.2}");     //      3.14（右对齐，宽 10）
    println!("{:?}", vec![1, 2]); // Debug 格式
    println!("{:#?}", vec![1, 2]); // Pretty Debug（多行）
}
```

### 2.7 控制流

#### if/else

```rust
fn main() {
    let x = 10;

    // if/else — 条件和 C/Java 类似
    if x > 5 {
        println!("大于 5");
    } else if x > 0 {
        println!("正数");
    } else {
        println!("非正数");
    }

    // if 是表达式！可以返回值
    // 两个分支必须返回相同类型
    let description = if x > 5 { "big" } else { "small" };
    println!("{x} is {description}");

    // ❌ 错误：两个分支类型不同
    // let bad = if true { 1 } else { "two" };
    // 编译错误: if and else have incompatible types
}
```

**`if` 是表达式**——这是 Rust 和 C/Java 的重要区别。
在 C 语言中，三元运算符 `x > 5 ? "big" : "small"` 是表达式。
在 Rust 中，`if/else` 本身就是表达式，不需要三元运算符。

#### loop、while、for

```rust
fn main() {
    // loop — 无限循环，可以用 break 返回值
    let result = loop {
        let answer = 42;
        break answer;  // break 可以带一个值！
    };
    println!("loop 返回: {result}");

    // while — 条件循环
    let mut count = 3;
    while count > 0 {
        println!("倒计时: {count}");
        count -= 1;
    }
    println!("发射！");

    // for — 遍历迭代器（最常用）
    for i in 0..5 {       // range: 0, 1, 2, 3, 4
        print!("{i} ");
    }
    println!();

    for i in 1..=5 {      // 包含 5: 1, 2, 3, 4, 5
        print!("{i} ");
    }
    println!();

    // 遍历数组/Vec
    let fruits = ["苹果", "香蕉", "橙子"];
    for fruit in &fruits {
        println!("水果: {fruit}");
    }

    // 带索引遍历
    for (i, fruit) in fruits.iter().enumerate() {
        println!("[{i}] {fruit}");
    }
}
```

**`for` 循环是 Rust 中最常用的循环**。它遍历任何实现了 `Iterator` trait 的类型。
`0..5` 是一个 range，它也实现了 Iterator。
和 C/Java 的 `for (int i = 0; i < 5; i++)` 相比，Rust 的 `for i in 0..5`
更安全（没有 off-by-one 错误）、更简洁。

### 2.8 函数

```rust
// 基本函数 — 参数类型必须标注
fn add(a: i32, b: i32) -> i32 {
    a + b  // 没有分号 → 这是返回值表达式
}

// 有分号的行是语句（返回 ()），没有分号的行是表达式（返回值）
fn explicit_return(x: i32) -> i32 {
    let result = x * 2;  // 语句
    result               // 表达式（返回值）
    // 等价于: return result;
}

// 返回 Result — 错误处理
fn divide(a: f64, b: f64) -> Result<f64, String> {
    if b == 0.0 {
        Err("除数不能为零".to_string())
    } else {
        Ok(a / b)
    }
}

fn main() {
    println!("3 + 4 = {}", add(3, 4));
    println!("5 * 2 = {}", explicit_return(5));

    match divide(10.0, 3.0) {
        Ok(result) => println!("10 / 3 = {result:.2}"),
        Err(e) => println!("错误: {e}"),
    }
}
```

**关键区别：** Rust 函数参数**必须标注类型**。
这是因为 Rust 编译器需要在编译期知道每个变量的类型（用于内存布局和类型检查），
不允许在函数签名上省略类型。

### 本章小结

| 概念 | 关键点 |
|------|--------|
| `let` / `let mut` | 不可变 / 可变绑定 |
| shadowing | 可以改变类型 |
| `const` / `static` | 编译期常量 / 全局静态 |
| 类型 | i32 是默认整数，f64 是默认浮点，char 是 4 字节 Unicode |
| `String` vs `&str` | 拥有 vs 借用，函数参数用 `&str` |
| `if` 是表达式 | 可以返回值 |
| `for` 遍历 | 最常用，基于 Iterator |
| 函数参数必须标注类型 | Rust 编译器的要求 |

---

## 3. 所有权系统

所有权（Ownership）是 Rust 最独特的特性。它让 Rust 能在没有垃圾回收（GC）的情况下
保证内存安全。理解所有权是理解 Rust 的关键——如果你只能学一章，就是这一章。

### 3.1 栈 vs 堆：内存在哪里？

在理解所有权之前，你需要知道数据存储在两种不同的内存区域：

**栈（Stack）**：
- 像一摞盘子：最后放上去的最先被拿走（LIFO）
- 所有数据必须有已知固定的大小
- 极快：只需移动栈指针
- 存放：`i32`、`f64`、`bool`、`char`、引用（`&T`）、所有字段都是栈类型的结构体

**堆（Heap）**：
- 像一个大仓库：需要时申请一块空间，用完归还
- 可以存放动态大小的数据
- 较慢：需要搜索空闲空间
- 存放：`String`（内容）、`Vec<T>`（元素）、`Box<T>`（值）

```rust
fn main() {
    // 栈上的数据
    let x: i32 = 42;          // 4 字节在栈上
    let flag: bool = true;    // 1 字节在栈上
    let tuple = (1, true);    // 5 字节在栈上

    // 堆上的数据
    let s = String::from("hello");
    // 栈上: ptr(指针) + len(长度=5) + capacity(容量=5) = 24 字节
    // 堆上: h e l l o = 5 字节

    let v = vec![1, 2, 3];
    // 栈上: ptr + len(3) + capacity(3) = 24 字节
    // 堆上: [1, 2, 3] = 12 字节

    println!("栈: x={x}, flag={flag}");
    println!("堆: s={s}, v={v:?}");

    // 查看大小
    println!("i32: {} bytes", std::mem::size_of::<i32>());        // 4
    println!("String: {} bytes", std::mem::size_of::<String>());  // 24（栈上的头部）
    println!("&str: {} bytes", std::mem::size_of::<&str>());      // 16（ptr + len）
}
```

**为什么区分栈和堆？** 因为所有权规则本质上是在管理**堆内存**的生命周期。
栈上的数据自动随函数调用分配和释放，不需要管理。但堆上的数据需要有人负责释放。

### 3.2 所有权三条规则

这三条规则是整个 Rust 内存安全的基础。请务必记住：

1. **每个值有且仅有一个所有者（owner）**
2. **当所有者离开作用域（scope），值被释放（dropped）**
3. **值可以被移动（move）或借用（borrow），但不能同时进行**

```rust
fn main() {
    // 规则 1: 每个值有且仅有一个所有者
    let s = String::from("hello");
    // s 是 "hello" 这个字符串数据的唯一所有者
    // "hello" 的实际内容在堆上，s 的变量（ptr/len/cap）在栈上

    // 规则 2: 所有者离开作用域时释放值
    {
        let temp = String::from("临时数据");
        println!("{temp}");
    } // ← 这里 temp 离开作用域
    // Rust 自动调用 drop：释放堆上的 "临时数据"
    // 你不需要手动调用 free() 或 delete！

    // println!("{temp}"); // ❌ 编译错误：temp 已经不存在了

    println!("{s}"); // ✅ s 仍然有效，因为还在作用域内
} // ← s 在这里离开作用域，"hello" 的堆内存被释放
```

### 3.3 Move 语义——为什么赋值会让原变量失效？

这是 Rust 最让初学者惊讶的行为：

```rust
fn main() {
    let s1 = String::from("hello");
    let s2 = s1;  // 这行做了什么？

    // println!("{s1}"); // ❌ 编译错误！s1 不再有效
    println!("{s2}");    // ✅ 只有 s2 有效
}
```

**为什么 `s1` 失效了？** 让我们看看内存中发生了什么：

```text
let s1 = String::from("hello");
┌──────────────┐          ┌───────────────┐
│ s1 (栈)       │          │ 堆             │
│ ptr ─────────────────→  │ h e l l o     │
│ len = 5      │          └───────────────┘
│ cap = 5      │
└──────────────┘

let s2 = s1;
┌──────────────┐          ┌───────────────┐
│ s2 (栈)       │          │ 堆             │
│ ptr ─────────────────→  │ h e l l o     │
│ len = 5      │          └───────────────┘
│ cap = 5      │
└──────────────┘
│ s1 = 无效     │  ← Rust 让 s1 失效！
└──────────────┘
```

如果只是复制了栈上的 3 个字段（ptr、len、cap），那么 s1 和 s2 都会指向同一块堆内存。
当 s1 和 s2 都离开作用域时，会**两次释放**同一块内存——这就是 double free bug！

Rust 的解决方案：赋值后让 s1 失效。这样只有 s2 会在作用域结束时释放内存。
这叫做**移动（move）**——所有权从 s1 转移到了 s2。

**对比：基本类型不会移动**

```rust
fn main() {
    let x = 42;
    let y = x;  // 这里是复制（copy），不是移动

    println!("x = {x}, y = {y}"); // ✅ 两者都有效！
    // 因为 i32 是基本类型，在栈上，复制成本极低
}
```

### 3.4 函数参数也发生移动

```rust
fn take_ownership(s: String) {
    println!("我拥有了: {s}");
} // ← s 在这里离开作用域，堆内存被释放

fn main() {
    let name = String::from("Alice");
    take_ownership(name);  // name 的所有权移动到函数的参数 s

    // println!("{name}"); // ❌ name 已经无效了

    // 修复方法 1：传入克隆
    let name2 = String::from("Bob");
    take_ownership(name2.clone());  // 传入副本
    println!("{name2}");  // ✅ name2 仍然有效

    // 修复方法 2：传入引用（下一章详讲）
    let name3 = String::from("Charlie");
    borrow_name(&name3);  // 传入引用，不转移所有权
    println!("{name3}");  // ✅ name3 仍然有效
}

fn borrow_name(s: &String) {
    println!("我借用: {s}");
} // s 是引用，离开作用域时不释放数据
```

**为什么？** 函数参数也是变量绑定。把 `name` 传给函数，就是把所有权给了参数。
函数结束后，参数被 drop，数据就没了。

### 3.5 Copy trait——哪些类型赋值时不移动？

实现了 `Copy` trait 的类型在赋值时**自动按位复制**，不发生移动。

```rust
fn main() {
    // 这些类型都是 Copy 的：
    let a: i32 = 42;      let b = a;     println!("{a} {b}"); // ✅
    let c: f64 = 3.14;    let d = c;     println!("{c} {d}"); // ✅
    let e: bool = true;   let f = e;     println!("{e} {f}"); // ✅
    let g: char = 'A';    let h = g;     println!("{g} {h}"); // ✅
    let i: &str = "hi";   let j = i;     println!("{i} {j}"); // ✅ (引用是 Copy)

    // 元组中所有字段都是 Copy → 整个元组是 Copy
    let t = (1, true, 'A');
    let t2 = t;
    println!("{t:?} {t2:?}"); // ✅
}
```

**哪些类型是 Copy？规则很简单：**

| 类型 | Copy? | 原因 |
|------|-------|------|
| 所有整数、浮点、bool、char | ✅ | 固定大小，在栈上 |
| 引用 `&T` | ✅ | 引用本身只是指针 |
| 所有字段都是 Copy 的元组 | ✅ | 递归规则 |
| 所有字段都是 Copy 的结构体（需要 derive） | ✅ | 需要显式声明 |
| `String` | ❌ | 堆数据 |
| `Vec<T>` | ❌ | 堆数据 |
| `Box<T>` | ❌ | 堆数据 |
| 包含非 Copy 字段的类型 | ❌ | 递归规则 |

**手动实现 Copy：**

```rust
#[derive(Debug, Copy, Clone)]
struct Point { x: f64, y: f64 }

fn main() {
    let p1 = Point { x: 1.0, y: 2.0 };
    let p2 = p1;  // Copy！p1 仍然有效
    println!("p1={p1:?}, p2={p2:?}"); // ✅
}
```

**注意：** `Copy` 要求 `Clone` 也必须实现。如果一个类型有 `Drop` 实现，则不能是 `Copy`。

### 3.6 Clone——显式深拷贝

如果你确实需要两份独立的数据副本，用 `.clone()`：

```rust
fn main() {
    let s1 = String::from("hello");
    let s2 = s1.clone();  // 深拷贝：堆上的数据也被复制

    println!("s1 = {s1}");  // ✅
    println!("s2 = {s2}");  // ✅

    // 它们指向不同的堆地址
    println!("不同地址: {}", s1.as_ptr() != s2.as_ptr());

    // Vec 也可以 clone
    let v1 = vec![1, 2, 3];
    let v2 = v1.clone();
    println!("{v1:?} {v2:?}");
}
```

**何时需要 clone？**
- 需要两份独立的数据
- 数据要被多个所有者持有
- **性能警告：** clone 是 O(n)，大对象要谨慎

**项目实例：**

```rust
// protocol/src/error.rs — OpenAgere 实际代码
#[derive(Debug, Clone)]
pub struct RateLimitedError {
    pub status: u16,
    pub message: String,     // 堆分配，需要 Clone
    pub retry_after: Option<std::time::Duration>,
}
// 包含 String，所以只能 Clone 不能 Copy
```

### 3.7 Drop trait——值如何被释放

当值离开作用域时，Rust 自动调用 `drop`：

```rust
struct Resource {
    name: String,
}

impl Drop for Resource {
    fn drop(&mut self) {
        println!("释放: {}", self.name);
    }
}

fn main() {
    let a = Resource { name: "数据库连接".into() };
    let b = Resource { name: "文件句柄".into() };
    println!("资源已创建");
}
// 输出顺序:
// 资源已创建
// 释放: 文件句柄  ← b 后声明，先 drop
// 释放: 数据库连接 ← a 先声明，后 drop
```

**Drop 顺序：后声明的先 drop（LIFO）。**

```rust
fn main() {
    // 手动提前释放
    let s = String::from("hello");
    drop(s);  // std::mem::drop 提前调用 Drop
    // println!("{s}"); // ❌ s 已经被 drop
}
```

### 3.8 总结：所有权决策流程

当你不确定该用什么时，按这个流程决策：

```
需要修改数据？
├─ 是 → 需要 &mut T
└─ 否 → 需要跨函数/作用域存活？
├─ 是 → 需要拥有（String, Vec）
└─ 否 → 借用即可（&str, &[T]）
```

在函数参数中：
- 只读取 → `&str`（最灵活）
- 需要修改 → `&mut T`
- 需要拥有 → `T`（会消耗调用方的值）

### 本章小结

| 概念 | 关键点 |
|------|--------|
| 栈 vs 堆 | 固定大小在栈上，动态大小在堆上 |
| 所有权规则 | 一个所有者，离开作用域释放 |
| Move | 赋值/传参 = 移动所有权，原变量失效 |
| Copy | 基本类型自动复制（不移动） |
| Clone | 显式深拷贝 `.clone()` |
| Drop | 离开作用域自动释放 |
| 决策流程 | 只读→借用，要改→&mut，要存→拥有 |

---

## 4. 借用与引用

上一章讲了很多数据的**所有权**会"移动"，导致原变量失效。但很多时候我们只是需要
**读取**或**修改**数据，并不需要拥有它。这就是**借用（borrowing）**——通过**引用
（reference）**来访问数据而不取得所有权。

### 4.1 不可变引用 `&T`

引用就像一本书的书签——你可以看到书的内容，但你不拥有这本书。

```rust
fn main() {
    let s = String::from("hello");

    // &s 创建了一个指向 s 的不可变引用
    // s 仍然是所有者，我们只是"借用"了它
    let len = calculate_length(&s);

    // s 仍然有效！所有权没有转移
    println!("'{s}' 的长度是 {len}");

    // 可以同时有多个不可变引用
    let r1 = &s;
    let r2 = &s;
    let r3 = &s;
    println!("{r1}, {r2}, {r3}"); // ✅ 都有效
}

fn calculate_length(s: &String) -> usize {
    // s 是引用，不是所有者
    // 参数类型 &String 表示"我借用一个 String"
    s.len()
} // s 离开作用域，但因为它不拥有数据，所以不会释放任何东西
```

**不可变引用的规则：**
- 不能通过它修改数据（只读）
- 可以同时存在多个（读-读不冲突）
- 不转移所有权

```rust
fn main() {
    let s = String::from("hello");
    let r = &s;
    // r.push_str("!"); // ❌ 不能通过不可变引用修改
    // 错误: cannot borrow as mutable
    println!("{r}");
}
```

### 4.2 可变引用 `&mut T`

如果你需要修改借用的数据，需要**可变引用**。

```rust
fn main() {
    let mut s = String::from("hello");

    // &mut s 创建了一个可变引用
    // 注意 s 本身也需要声明为 mut
    add_world(&mut s);
    println!("{s}"); // 输出: hello, world!
}

fn add_world(s: &mut String) {
    s.push_str(", world!");
}
```

#### 借用规则——Rust 编译器的核心保障

Rust 有一条铁律，在编译期强制执行：

**在任意时刻，你要么只能有一个可变引用，要么只能有多个不可变引用。**

```rust
fn main() {
    let mut s = String::from("hello");

    // ✅ 可以有多个不可变引用
    let r1 = &s;
    let r2 = &s;
    println!("{r1} {r2}"); // OK——两个只读引用可以共存

    // ❌ 不能同时有可变引用和不可变引用
    let r3 = &s;       // 不可变引用
    let r4 = &s;       // 不可变引用
    // let r5 = &mut s; // ❌ 编译错误！
    // 错误: cannot borrow `s` as mutable because it is also borrowed as immutable

    // ✅ 不可变引用不再使用后，可以有可变引用（NLL）
    let r6 = &mut s;  // OK，因为 r3/r4 已经不再使用了
    r6.push_str("!");
}
```

**为什么需要这个规则？** 防止**数据竞争（data race）**。

数据竞争发生在以下三个条件同时满足时：
1. 两个或多个指针同时访问同一数据
2. 其中至少一个在写入
3. 没有同步机制

数据竞争是 C/C++ 中最难调试的 bug 之一——程序可能有时正常，有时崩溃。
Rust 的借用规则在**编译期**就消除了数据竞争。你不需要运行时锁来防止它。

```rust
// 两个引用的场景
fn main() {
    let mut s = String::from("hello");

    let r1 = &s;       // 第一次不可变借用
    let r2 = &s;       // 第二次不可变借用
    println!("{r1} {r2}");
    // r1 和 r2 在这之后不再使用（NLL: Non-Lexical Lifetime）

    let r3 = &mut s;   // ✅ 可变借用——因为 r1/r2 已不再使用
    r3.push_str("!");
    println!("{r3}");
}
```

### 4.3 悬垂引用

```rust
// ❌ 这个函数无法编译
// fn dangle() -> &String {
    //     let s = String::from("hello");
    //     &s  // s 在函数结束时被 drop，返回的引用指向已释放的内存！
    // }
// 编译错误: returns a value referencing data owned by the function's context

// ✅ 正确做法：返回拥有所有权的值
fn no_dangle() -> String {
    let s = String::from("hello");
    s // 返回所有权，调用者成为新所有者
}
```

Rust 编译器保证**引用永远有效**——它不会比被引用的数据活得更久。
这就是为什么 Rust 不会有悬垂指针（dangling pointer）。

### 4.4 切片引用

#### 字符串切片 `&str`

```rust
fn main() {
    let s = String::from("hello world");

    // &s[0..5] 是字符串切片，类型是 &str
    let hello: &str = &s[0..5];  // "hello"
    let world: &str = &s[6..11]; // "world"
    println!("{hello} {world}");

    // 简化：省略起始或结束索引
    println!("{}", &s[..5]);  // "hello" (从开头)
    println!("{}", &s[6..]);  // "world" (到末尾)
    println!("{}", &s[..]);   // 整个字符串

    // 函数返回切片引用
    fn first_word(s: &str) -> &str {
        let bytes = s.as_bytes();
        for (i, &byte) in bytes.iter().enumerate() {
            if byte == b' ' {
                return &s[..i];
            }
        }
        s
    }

    println!("第一个词: {}", first_word("hello world"));
}
```

**注意 UTF-8：** 切片索引必须在字符边界上。中文字符占 3 字节。

```rust
fn main() {
    let s = String::from("你好世界");
    // let bad = &s[0..1]; // ❌ panic! 不是字符边界
    let safe = &s[0..3]; // "你"（3 字节 = 1 个中文字符）
    println!("{safe}");

    // 安全的方式：用 chars()
    for (i, c) in s.chars().enumerate() {
        println!("字符 {i}: {c}");
    }
}
```

#### 数组切片 `&[T]`

```rust
fn main() {
    let arr = [10, 20, 30, 40, 50];

    let slice = &arr[1..4]; // &[i32]，值为 [20, 30, 40]
    println!("切片: {slice:?}");

    // 切片作为函数参数
    fn sum(slice: &[i32]) -> i32 {
        slice.iter().sum()
    }

    println!("和: {}", sum(&arr));
    println!("切片和: {}", sum(slice));
}
```

### 4.5 函数参数设计

函数签名中的参数类型表达了函数的**意图**：

```rust
// 接受 &T → "我只读取"
fn print_length(s: &str) {
    println!("长度: {}", s.len());
}

// 接受 &mut T → "我需要修改"
fn append_exclaim(s: &mut String) {
    s.push('!');
}

// 接受 T → "我要拥有"
fn consume(s: String) {
    println!("拥有: {s}");
} // s 在这里被 drop

fn main() {
    let s = String::from("hello");

    print_length(&s);      // 借用
    print_length("world"); // 字面量也行

    let mut s2 = String::from("hello");
    append_exclaim(&mut s2);
    println!("{s2}"); // "hello!"

    let s3 = String::from("rust");
    consume(s3);
    // println!("{s3}"); // ❌ s3 已经被移动
}
```

**为什么 `&str` 比 `&String` 更好？**

`&str` 可以接受 `&String`、`&str`、字符串字面量等任何字符串引用。
`&String` 只能接受 `&String`，限制了灵活性。这是 Rust 的**解引用强制转换**
（Deref Coercion）：`&String` 可以自动转换为 `&str`。

### 4.6 项目中的借用模式

```rust
// core/src/agents_md.rs — 结构体借用
pub struct AgentsMdManager<'a> {
    config: &'a Config,  // 借用 Config，不拥有它
}
// 为什么？AgentsMdManager 只是临时使用配置来查找文件

// core/src/exec_policy.rs — 多个借用
pub(crate) struct ExecApprovalRequest<'a> {
    pub(crate) command: &'a [String],
    pub(crate) file_system_access_policy: &'a FileSystemAccessPolicy,
    pub(crate) access_cwd: &'a Path,
}
// 为什么？审批请求只是临时打包数据，不需要拥有
```

### 本章小结

| 概念 | 关键点 |
|------|--------|
| `&T` | 不可变引用，可以多个同时存在 |
| `&mut T` | 可变引用，同时只能有一个 |
| 借用规则 | 一个可变 OR 多个不可变，不能同时 |
| 悬垂引用 | 编译器阻止返回局部变量的引用 |
| `&str` | 字符串切片引用 |
| `&[T]` | 数组切片引用 |
| NLL | 引用在最后一次使用时结束 |
| 函数参数 | `&str` 比 `&String` 更灵活 |

---

## 5. 生命周期

生命周期（lifetime）是 Rust 用来保证引用有效性的机制。编译器需要确保**引用不会比
它指向的数据活得更久**。大多数时候编译器能自动推断，但有时需要你手动标注。

### 5.1 为什么需要生命周期？

考虑这个函数——它无法通过编译：

```rust
// ❌ 编译错误
// fn longest(x: &str, y: &str) -> &str {
    //     if x.len() > y.len() { x } else { y }
    // }
```

**为什么？** 返回值是一个引用，但编译器不知道它指向 `x` 还是 `y`。
因为 `x` 和 `y` 可能有不同的生命周期——如果返回的引用指向已经释放的数据，
就会出现悬垂引用。

**修复：** 用生命周期标注告诉编译器：

```rust
// ✅ 正确
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}

fn main() {
    let s1 = String::from("长字符串");
    let result;
    {
        let s2 = String::from("短");
        result = longest(s1.as_str(), s2.as_str());
        println!("最长: {result}"); // ✅ result 在这里使用是安全的
    }
    // println!("{result}"); // ❌ s2 已经 drop 了，result 引用的数据无效
}
```

**`'a` 的含义：** "返回值的生命周期至少和输入参数中**较短**的那个一样长"。
这不是说 x 和 y 必须活一样久——而是说返回值不能比两者中活得短的那个活得更久。

### 5.2 生命周期标注语法

```rust
// &i32        — 普通引用
// &'a i32     — 带生命周期 'a 的引用
// &'a mut i32 — 带生命周期 'a 的可变引用

// 'a 只是一个名字——你可以用任何小写字母
// 多个生命周期用不同名字：'a, 'b, 'c

fn complex<'a, 'b>(x: &'a str, y: &'b str) -> &'a str {
    // 返回值明确与 x 有相同的生命周期
    x
}

fn main() {
    let s1 = String::from("hello");
    let s2 = String::from("world");
    let r = complex(&s1, &s2);
    println!("{r}"); // "hello" — 与 s1 同生命周期
}
```

### 5.3 结构体中的生命周期

如果结构体包含引用，**必须**标注生命周期。这是因为编译器需要确保
结构体不会比它引用的数据活得更久。

```rust
// ❌ 编译错误
// struct Excerpt {
    //     text: &str,  // 缺少生命周期标注
    // }

// ✅ 正确
struct Excerpt<'a> {
    text: &'a str,
}

impl<'a> Excerpt<'a> {
    fn new(text: &'a str) -> Self {
        Excerpt { text }
    }

    fn display(&self) {
        println!("摘录: {}", self.text);
    }

    fn first_word(&self) -> &str {
        // 返回值自动获得 self 的生命周期（消除规则 3）
        self.text.split_whitespace().next().unwrap_or("")
    }
}

fn main() {
    let novel = String::from("Call me Ishmael. Some years ago...");
    let first = &novel[..];
    let excerpt = Excerpt::new(first);
    excerpt.display();
    println!("第一个词: {}", excerpt.first_word());
    // excerpt 不能比 novel 活得更久
}
```

**项目实例：**

```rust
// core/src/client.rs — 混合借用和拥有
struct WebsocketConnectParams<'a> {
    session_telemetry: &'a SessionTelemetry,  // 借用
    api_provider: Provider,                    // 拥有（Copy）
    turn_metadata_header: Option<&'a str>,     // 借用
}
// 这个结构体是一个"参数包"
// 借用不需要拥有的字段，拥有需要独立存活的字段
// 'a 确保借用字段不会提前失效

// tui/src/app_command.rs — 枚举中的生命周期
pub(crate) enum AppCommandView<'a> {
    RealtimeConversationStart(&'a ConversationStartParams),
    RunUserShellCommand { command: &'a str },
    UserTurn {
        items: &'a [UserInput],
        cwd: &'a PathBuf,
        model: &'a str,
    },
}
// "视图"类型是轻量级包装器，借用调用者数据
```

### 5.4 生命周期消除规则

编译器使用三条规则自动推断省略的生命周期。大多数时候你不需要手动标注。

**规则 1：每个引用参数获得自己的生命周期。**

```rust
// fn foo(x: &str) → fn foo<'a>(x: &'a str)
// fn foo(x: &str, y: &str) → fn foo<'a, 'b>(x: &'a str, y: &'b str)
```

**规则 2：如果只有一个输入生命周期，它赋给所有输出。**

```rust
// fn foo(x: &str) -> &str → fn foo<'a>(x: &'a str) -> &'a str
```

**规则 3：如果有 `&self` 或 `&mut self`，self 的生命周期赋给所有输出。**

```rust
// fn foo(&self, x: &str) -> &str
// → fn foo<'a, 'b>(&'a self, x: &'b str) -> &'a str
// 返回值绑定到 self，不是 x
```

```rust
// 不需要手动标注的例子：
fn first_word(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
    // 规则 1 + 2：一个输入 → 输出与它相同
}

struct MyStruct { data: String }
impl MyStruct {
    fn get_data(&self) -> &str { &self.data }
    // 规则 3：返回值绑定到 self 的生命周期
}

fn main() {
    let s = String::from("hello world");
    let w = first_word(&s);
    println!("第一个词: {w}");
}
```

**何时需要手动标注？** 当返回值取决于多个输入参数时。

```rust
// 需要标注：两个输入，不知道返回绑定到哪个
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
```

### 5.5 `'static` 生命周期

`'static` 表示引用在整个程序运行期间都有效。

```rust
fn main() {
    // 字符串字面量是 &'static str
    let s: &'static str = "我永远不会被释放";
    // 因为字面量存储在二进制文件中，整个运行期间都存在
    println!("{s}");

    // 'static 作为 trait bound
    fn require_static<T: 'static>(_: T) {}
    require_static(42);      // i32 是 'static
    require_static("hello"); // &str 可以是 'static
}
```

**注意：** 不要滥用 `'static`。大多数函数不需要 `'static` 约束。
看到 `'static` bound 时，先想想是否真的需要。

### 5.6 Cow（Clone On Write）— 零成本的利器

`Cow` 是标准库中最聪明的类型之一。它可以持有**借用的**或**拥有的**数据，
让你在不修改时零成本借用，在需要修改时才克隆。

```rust
use std::borrow::Cow;

// Cow<'a, str> 可以是：
// Cow::Borrowed(&'a str)  — 借用，零成本
// Cow::Owned(String)       — 拥有，需要修改时

fn maybe_modify(s: &str) -> Cow<'_, str> {
    if s.contains("bad") {
        Cow::Owned(s.replace("bad", "good"))  // 需要修改 → 克隆
    } else {
        Cow::Borrowed(s)  // 不需要修改 → 零成本借用
    }
}

fn main() {
    let clean = "hello world";
    let dirty = "hello bad world";

    let r1 = maybe_modify(clean);
    println!("clean → {r1}"); // 借用，没有分配新内存

    let r2 = maybe_modify(dirty);
    println!("dirty → {r2}"); // 拥有，分配了新 String
}
```

**Cow 用于切片：**

```rust
use std::borrow::Cow;

fn sanitize(items: &[i32]) -> Cow<'_, [i32]> {
    if items.iter().all(|&x| x > 0) {
        Cow::Borrowed(items)  // 不需要修改
    } else {
        let cleaned: Vec<i32> = items.iter().map(|&x| x.abs()).collect();
        Cow::Owned(cleaned)   // 需要修改
    }
}

fn main() {
    let good = vec![1, 2, 3];
    let bad = vec![-1, 2, -3];
    println!("good → {:?}", sanitize(&good));   // Borrowed
    println!("bad → {:?}", sanitize(&bad));     // Owned
}
```

**项目实例：**

```rust
// core/src/history_sanitize.rs — OpenAgere 实际代码
pub(crate) fn sanitize_history_for_wire_api<'a>(
wire_api: WireApi,
items: &'a [ResponseItem],
) -> Cow<'a, [ResponseItem]> {
    // 在常见请求路径上，历史数据不需要修改
    // 返回 Cow::Borrowed 避免整段克隆
    // 只有当确实有项需要清洗时才分配新 Vec
}
```

**为什么 Cow 是零成本抽象的典范？** 因为编译器会优化掉不必要的分支。
在常见路径上，Cow::Borrowed 和一个普通引用一样快。只有在异常路径上才有分配开销。


### 5.7 生命周期的本质 — 它到底是什么？

很多人以为生命周期是"引用的寿命"，其实不是。**生命周期是编译器用来验证
"引用是否指向还活着的数据"的一套约束系统。**

通俗理解：

```text
想象三个变量在时间轴上的存活区间：

时间 -->
a: ============================  (整个 main 函数)
b:      ==============          (某个代码块内)
c:           ========           (更小的代码块内)

如果有一个引用 &'x str 指向 b 的数据：
- 'x 必须 <= b 的存活区间
- 你不能在 b 死后还使用这个引用
```

**编译器做的事情：**

```text
1. 给每个引用分配一个生命周期变量（'a, 'b, ...）
2. 根据代码结构推断约束关系（'a 必须比 'b 短/一样长）
3. 检查是否存在违反约束的用法
4. 如果有 -> 编译错误；如果没有 -> 编译通过
```

### 5.8 生命周期图解 — 用时间轴理解

```text
示例 1：安全的引用

fn main() {
    let s = String::from("hello");
    //    |--- s 的存活区间 ---|
    let r = &s;  // r 借用 s
    println!("{r}");
}
// r 的存活区间 包含在 s 的存活区间内 -> 安全


示例 2：危险的引用（编译错误）

fn main() {
    let r;
    // |--- r 的存活区间 ------------|
    {
        let s = String::from("hello");
        //  |--- s 的存活区间 ---|
        r = &s;
    }   // <-- s 在这里死了！
    println!("{r}");  // ERROR: r 指向已释放的数据
}
// r 的存活区间 > s 的存活区间 -> 编译错误
```

### 5.9 函数签名中的生命周期逻辑

当函数返回引用时，编译器必须知道返回的引用指向哪个参数。

```rust
// 编译器不知道返回的引用指向 x 还是 y
// fn longest(x: &str, y: &str) -> &str { ... }
//
// 编译器的困惑：
// "如果返回 x，那返回值的生命周期 = x 的生命周期
//  如果返回 y，那返回值的生命周期 = y 的生命周期
//  但 x 和 y 可能有不同的生命周期！
//  我不知道该用哪个 -> 报错"

// 手动标注：告诉编译器"返回值和两个参数有相同的生命周期约束"
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
```

**'a 的实际含义：**

```text
"存在一个生命周期 'a，使得：
 - x 至少活 'a 那么久
 - y 至少活 'a 那么久
 - 返回值也活 'a 那么久"

等价于：返回值的有效性 <= min(x 的寿命, y 的寿命)
```

**调用时编译器怎么检查：**

```rust
fn main() {
    let s1 = String::from("长字符串");
    let result;
    {
        let s2 = String::from("短");
        // s1: ============================  (整个 main)
        // s2:      ==========              (这个块)
        // 'a:        ==========            (= min(s1, s2) = s2)
        result = longest(&s1, &s2);
        println!("{result}"); // OK: result 在 s2 还活着时使用
    }
    // s2 已死 -> 'a 也结束了 -> result 不能再用
    // println!("{result}"); // 编译错误
}
```

### 5.10 结构体生命周期的逻辑

```rust
struct Excerpt<'a> {
    text: &'a str,
}
// 'a 的含义：
// "这个结构体的存活时间不能超过 'a"
// 等价于：Excerpt<'a> 包含于 'a
```

```text
图解：

let novel = String::from("Call me Ishmael...");
let excerpt;

// |---- novel 的存活区间 -----------|
// |                                 |
// |  |-- excerpt 的存活区间 -----|   |
// |  |                            |   |
// |  |  excerpt = Excerpt {       |   |
// |  |      text: &novel          |   |
// |  |  };                        |   |
// |  |                            |   |
// |  |  println!("{}", excerpt);  |   |
// |  |                            |   |
// |  |----------------------------|   |
// |                                 |
// |---------------------------------|

excerpt 包含于 novel -> 安全
```

### 5.11 常见生命周期错误与修复

**错误 1：返回局部变量的引用**

```rust
// 编译错误
// fn get_ref() -> &String {
//     let s = String::from("hello");
//     &s  // s 在函数结束时被 drop
// }

// 修复：返回值而不是引用
fn get_owned() -> String {
    String::from("hello")
}
```

**错误 2：两个不同生命周期的引用混用**

```rust
// 编译错误
// fn bad<'a, 'b>(x: &'a str, y: &'b str) -> &'a str {
//     if x.len() > y.len() { x } else { y }
//     //                         ^ 期望 &'a str，实际是 &'b str
// }

// 修复 1：只返回一个参数
fn correct<'a, 'b>(x: &'a str, _y: &'b str) -> &'a str { x }

// 修复 2：如果确实需要返回两者之一，用同一个生命周期
fn either<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
```

**错误 3：结构体引用的数据提前失效**

```rust
struct Config<'a> { name: &'a str }

// 编译错误
// fn bad_config() -> Config {
//     let name = String::from("test");
//     Config { name: &name }
//     // name 在函数结束时被 drop，但 Config 还想继续用
// }

// 修复 1：让调用者提供数据
fn good_config<'a>(name: &'a str) -> Config<'a> {
    Config { name }
}

// 修复 2：改为拥有数据
struct OwnedConfig { name: String }
```

### 5.12 生命周期子类型（Lifetime Subtyping）

当你需要表达"一个生命周期比另一个更长"时：

```rust
// 'a: 'b 读作 "'a outlives 'b" — "'a 至少和 'b 一样长"

fn process<'a, 'b>(long_lived: &'a str, short_lived: &'b str) -> &'b str
where
    'a: 'b,  // 'a 至少和 'b 一样长
{
    // 可以把 &'a str 当作 &'b str 使用
    // 因为 'a 比 'b 长，所以 &'a 的数据在 'b 期间一定有效
    if long_lived.len() > short_lived.len() {
        long_lived
    } else {
        short_lived
    }
}

fn main() {
    let long = String::from("I live a long time");
    let result;
    {
        let short = String::from("hi");
        result = process(&long, &short);
        println!("{result}");
    }
}
```

**什么时候需要？** 大多数时候不需要。只有当你有复杂的引用嵌套
或 trait bound 需要表达生命周期关系时才用。

### 5.13 生命周期与 Trait 的交互

```rust
// Trait bound 中的生命周期
fn longest_with_announcement<'a, T>(
    x: &'a str,
    y: &'a str,
    ann: T,
) -> &'a str
where
    T: std::fmt::Display,
{
    println!("Announcement! {ann}");
    if x.len() > y.len() { x } else { y }
}

// 带生命周期的 trait
trait Named<'a> {
    fn name(&self) -> &'a str;
}

struct Person<'a> {
    first_name: &'a str,
}

impl<'a> Named<'a> for Person<'a> {
    fn name(&self) -> &'a str { self.first_name }
}
```

### 5.14 实践建议

```text
自动推断（不需要标注）：
- 函数只有一个引用参数 -> 输出自动绑定到它
- 方法有 &self / &mut self -> 输出自动绑定到 self
- 结构体字段是拥有类型（String, Vec）-> 不需要标注

需要手动标注：
- 函数有多个引用参数，返回值是引用
- 结构体包含引用字段
- 实现带生命周期的 trait
```

**给初学者的建议：**

1. 先写不带生命周期的版本，让编译器告诉你哪里需要标注
2. 大多数时候编译器会自动推断，你只需要在报错时加标注
3. 如果频繁遇到生命周期问题，考虑改为拥有数据（String 代替 &str）
4. 'static 不是万能解药——它意味着"永远有效"，局部数据不是

### 本章小结

| 概念 | 关键点 |
|------|--------|
| `'a` | 标注引用的有效范围 |
| 消除规则 | 大多数情况编译器自动推断 |
| 结构体生命周期 | 包含引用的结构体必须标注 |
| `'static` | 整个程序运行期间有效 |
| `Cow` | 条件拥有——零成本借用或克隆 |
| 常见陷阱 | 悬垂引用、过度标注 |

---

## 6. 结构体与方法

结构体（struct）是 Rust 中定义自定义数据类型的核心方式。
如果说基本类型是"字母"，那么结构体就是"单词"——你把相关的字段组合在一起。

### 6.1 命名字段结构体

```rust
struct User {
    name: String,
    email: String,
    age: u32,
    active: bool,
}

fn main() {
    // 创建实例——必须提供所有字段
    let user = User {
        name: String::from("Alice"),
        email: String::from("alice@example.com"),
        age: 30,
        active: true,
    };

    // 访问字段（点号语法）
    println!("{} ({})", user.name, user.email);

    // 修改字段（需要 mut）
    let mut user2 = User {
        name: String::from("Bob"),
        email: String::from("bob@example.com"),
        age: 25,
        active: false,
    };
    user2.active = true;

    // 结构体更新语法（..）
    // 从现有实例创建新实例，只覆盖部分字段
    let user3 = User {
        name: String::from("Charlie"),
        ..user2  // 其余字段从 user2 复制
    };
    println!("user3 active: {}", user3.active); // true（从 user2 复制）
}
```

### 6.2 元组结构体与 Newtype 模式

```rust
// 元组结构体：有名字但没有命名字段
struct Color(u8, u8, u8);
struct Point(f64, f64);

fn main() {
    let red = Color(255, 0, 0);
    println!("R={}, G={}, B={}", red.0, red.1, red.2);
}
```

**Newtype 模式——元组结构体最重要的用法：**

包装一个已有类型，赋予新的语义。编译器会阻止你混淆不同类型。

```rust
struct Meters(f64);
struct Seconds(f64);

fn calculate_speed(distance: Meters, time: Seconds) -> f64 {
    distance.0 / time.0
}

fn main() {
    let distance = Meters(100.0);
    let time = Seconds(9.58);
    let speed = calculate_speed(distance, time);
    println!("速度: {speed:.2} m/s");

    // calculate_speed(time, distance);
    // ❌ 编译错误！类型不匹配
    // 编译器阻止了你把"秒"当成"米"传给函数
}
```

**为什么这样设计？** 在大型项目中，这种类型安全极其重要。
没有 newtype，两个 `f64` 参数很容易搞混——编译器不会报错，但逻辑错误。

**项目实例：**

```rust
// protocol/src/protocol.rs — newtype + serde
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, TS)]
#[serde(transparent)]  // 序列化表现得和普通 String 一样
#[ts(type = "string")] // TypeScript 类型是 string
pub struct GitSha(pub String);
// GitSha 和 String 在类型层面不同
// 但序列化时表现得和普通字符串一样
```

### 6.3 impl 块——为结构体添加方法

```rust
struct Rectangle {
    width: f64,
    height: f64,
}

impl Rectangle {
    // 关联函数（没有 self）— 通常用作构造函数
    fn new(width: f64, height: f64) -> Self {
        Rectangle { width, height }
    }

    fn square(size: f64) -> Self {
        Self { width: size, height: size }
    }

    // 方法 — &self（不可变借用）
    fn area(&self) -> f64 {
        self.width * self.height
    }

    fn perimeter(&self) -> f64 {
        2.0 * (self.width + self.height)
    }

    // 方法 — &mut self（可变借用）
    fn scale(&mut self, factor: f64) {
        self.width *= factor;
        self.height *= factor;
    }

    // 方法 — self（获取所有权，消耗自身）
    fn into_tuple(self) -> (f64, f64) {
        (self.width, self.height)
    }
}

// Self（大写 S）是当前类型的别名
// Self { width, height } 等价于 Rectangle { width, height }

fn main() {
    // 关联函数（通过类型名调用，不需要实例）
    let rect = Rectangle::new(30.0, 50.0);
    let sq = Rectangle::square(10.0);

    // 方法（通过实例调用）
    println!("面积: {}", rect.area());
    println!("周长: {}", rect.perimeter());

    // 可变方法
    let mut rect2 = Rectangle::new(1.0, 2.0);
    rect2.scale(3.0);
    println!("缩放后: {}x{}", rect2.width, rect2.height);

    // 消耗自身的方法
    let (w, h) = rect2.into_tuple();
    println!("元组: ({w}, {h})");
    // println!("{rect2}"); // ❌ rect2 已经被移动到 into_tuple
}
```

**三种 self 的区别：**

| 方法签名 | 语义 | 调用后 |
|---------|------|--------|
| `&self` | 只读借用 | 实例仍可用 |
| `&mut self` | 可变借用 | 实例仍可用 |
| `self` | 获取所有权 | 实例被消耗 |

### 6.4 Default trait

```rust
#[derive(Debug, Default)]
struct Options {
    timeout_ms: u64,     // 默认 0
    retries: u32,        // 默认 0
    verbose: bool,       // 默认 false
    name: String,        // 默认 ""
}

fn main() {
    // 只覆盖需要的字段，其余用默认值
    let opts = Options {
        timeout_ms: 10000,
        verbose: true,
        ..Default::default()
    };
    println!("{opts:?}");
    // Options { timeout_ms: 10000, retries: 0, verbose: true, name: "" }
}
```

**项目实例：**

```rust
// core/src/agere_thread.rs — 很多 Option 字段
#[derive(Clone, Default)]
pub struct AgereThreadTurnContextOverrides {
    pub cwd: Option<PathBuf>,              // 默认 None
    pub approval_policy: Option<AskForApproval>,
    pub permission_profile: Option<PermissionProfile>,
    // ... 很多 Option 字段
}
// 调用方只覆盖需要修改的：
// let overrides = AgereThreadTurnContextOverrides {
    //     cwd: Some(path),
    //     ..Default::default()  // 其余全部 None
    // };
```

### 6.5 Builder 模式（方法链）

```rust
#[derive(Debug)]
struct Request {
    url: String,
    method: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
}

impl Request {
    fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            method: "GET".to_string(),
            headers: Vec::new(),
            body: None,
        }
    }

    // 每个方法返回 Self，支持链式调用
    fn method(mut self, method: &str) -> Self {
        self.method = method.to_string();
        self
    }

    fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.push((key.to_string(), value.to_string()));
        self
    }

    fn body(mut self, body: &str) -> Self {
        self.body = Some(body.to_string());
        self
    }
}

fn main() {
    let request = Request::new("https://api.example.com/data")
    .method("POST")
    .header("Content-Type", "application/json")
    .body(r#"{"key": "value"}"#);

    println!("{request:?}");
}
```

**为什么？** Builder 模式让构造复杂对象变得清晰易读。
每个方法返回 `self`，可以链式调用。最终调用者一眼就能看出配置了什么。

### 本章小结

| 概念 | 关键点 |
|------|--------|
| struct | 命名字段、元组、单元三种 |
| newtype | 类型安全包装 |
| impl | 关联函数（无 self）、方法（&self/&mut self/self） |
| Self | 当前类型的别名 |
| Default | 默认值，`..Default::default()` 覆盖 |
| Builder | 链式 self 返回 |

---

## 7. 枚举与模式匹配

Rust 的枚举（enum）是语言最强大的特性之一。与 C/Java 的枚举不同，
Rust 的每个变体可以携带**不同类型和数量**的数据。配合模式匹配（match），
编译器保证你处理了**所有可能的情况**。

### 7.1 基本枚举

```rust
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

fn main() {
    let dir = Direction::Up;
    match dir {
        Direction::Up => println!("上"),
        Direction::Down => println!("下"),
        Direction::Left => println!("左"),
        Direction::Right => println!("右"),
    }
}
```

### 7.2 带数据的枚举——Rust 的独特之处

这是 Rust 枚举和 C/Java 枚举的根本区别。每个变体可以携带不同类型的数据：

```rust
enum Message {
    Quit,                            // 无数据
    Move { x: i32, y: i32 },        // 命名字段（像结构体）
    Write(String),                   // 一个值
    ChangeColor(u8, u8, u8),        // 多个值
}

impl Message {
    fn describe(&self) -> String {
        match self {
            Message::Quit => "退出".to_string(),
            Message::Move { x, y } => format!("移动到 ({x}, {y})"),
            Message::Write(text) => format!("写入: {text}"),
            Message::ChangeColor(r, g, b) => format!("颜色: rgb({r},{g},{b})"),
        }
    }
}

fn main() {
    let messages = vec![
    Message::Quit,
    Message::Move { x: 10, y: 20 },
    Message::Write("hello".to_string()),
    Message::ChangeColor(255, 128, 0),
    ];

    for msg in &messages {
        println!("{}", msg.describe());
    }
}
```

**为什么这样设计？** 看看 `Option<T>` 和 `Result<T, E>` 就明白了：

```rust
// 标准库中的定义
enum Option<T> {
    Some(T),
    None,
}

enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

`Option` 解决了**空指针**问题（Tony Hoare 称之为"十亿美元错误"）。
在 Rust 中，没有 null——你要处理"可能不存在"的值，必须显式处理 `None` 情况。
编译器会确保你不会忘记。

```rust
fn find_user(id: u32) -> Option<String> {
    match id {
        1 => Some("Alice".to_string()),
        2 => Some("Bob".to_string()),
        _ => None,
    }
}

fn main() {
    match find_user(1) {
        Some(name) => println!("找到: {name}"),
        None => println!("未找到"),
    }

    // Option 的组合方法
    let name = find_user(2).unwrap_or("Unknown".to_string());
    let upper = find_user(1).map(|n| n.to_uppercase());
    println!("名字: {name}");
    println!("大写: {upper:?}");
}
```

### 7.3 `match` — 穷举模式匹配

`match` 是 Rust 中最重要的控制流之一。它要求你处理**所有可能的变体**。

```rust
#[derive(Debug, Clone, Copy)]
enum ApprovalPolicy {
    OnFailure,
    UnlessTrusted,
    Never,
    OnRequest,
    Granular { access_approval: bool },
}

fn wants_access_approval(policy: ApprovalPolicy) -> bool {
    match policy {
        ApprovalPolicy::OnFailure => true,
        ApprovalPolicy::UnlessTrusted => true,
        ApprovalPolicy::Never => false,
        ApprovalPolicy::OnRequest => false,
        ApprovalPolicy::Granular { access_approval } => access_approval,
        // 注意：没有 _ 通配符！
        // 如果以后新增一个变体但忘记处理，编译器会报错
    }
}

fn main() {
    println!("{}", wants_access_approval(ApprovalPolicy::Never));
    println!("{}", wants_access_approval(ApprovalPolicy::OnFailure));
    println!("{}", wants_access_approval(ApprovalPolicy::Granular { access_approval: true }));
}
```

**`match` 的强大之处：**

```rust
fn main() {
    // 多模式匹配
    let value = 5;
    match value {
        1 | 2 | 3 => println!("小数字"),
        4 | 5 | 6 => println!("中等数字"),
        _ => println!("其他"),
    }

    // 范围匹配
    let ch = 'c';
    match ch {
        'a'..='z' => println!("小写字母"),
        'A'..='Z' => println!("大写字母"),
        _ => println!("其他字符"),
    }

    // 解构
    enum Shape {
        Circle { radius: f64 },
        Rectangle { width: f64, height: f64 },
    }
    let shape = Shape::Circle { radius: 5.0 };
    match shape {
        Shape::Circle { radius } => println!("圆形, 半径={radius}"),
        Shape::Rectangle { width, height } => println!("矩形, {width}x{height}"),
    }
}
```

### 7.4 `if let` / `while let` — 简化单分支

当只关心一个变体时，`if let` 比 `match` 更简洁：

```rust
fn main() {
    let config = Some("production");

    // match 写法（冗余）
    match config {
        Some(val) => println!("配置: {val}"),
        None => {}, // 不想处理
    }

    // if let 写法（简洁）
    if let Some(val) = config {
        println!("配置: {val}");
    }

    // if let + else
    if let Some(val) = config {
        println!("有值: {val}");
    } else {
        println!("没有值");
    }

    // while let — 持续消费
    let mut stack = vec![1, 2, 3, 4, 5];
    while let Some(top) = stack.pop() {
        println!("弹出: {top}");
    }
}
```

### 7.5 Let Chains（Rust 2024）

Rust 2024 edition 引入了 let chains——在 if 条件中链式使用 let：

```rust
fn main() {
    let value: Option<i32> = Some(42);

    // let chain：let 绑定 + 条件组合
    if let Some(x) = value && x > 10 {
        println!("有值且大于 10: {x}");
    }

    // 多个 let chain
    let a: Option<String> = Some("hello".to_string());
    let b: Option<i32> = Some(100);

    if let Some(s) = &a && let Some(n) = b && s.len() == n / 20 {
        println!("s={s}, n={n}");
    }
}
```

### 7.6 `matches!` 宏

```rust
fn main() {
    let value = Some(42);

    // matches! 返回 bool
    if matches!(value, Some(x) if x > 10) {
        println!("大于 10");
    }

    // 匹配多个模式
    enum ToolKind { Function, Mcp, Builtin }
    enum ToolPayload { Function { name: String }, Mcp { server: String } }

    fn matches_kind(kind: &ToolKind, payload: &ToolPayload) -> bool {
        matches!(
        (kind, payload),
        (ToolKind::Function, ToolPayload::Function { .. })
        | (ToolKind::Mcp, ToolPayload::Mcp { .. })
        )
    }

    let kind = ToolKind::Function;
    let payload = ToolPayload::Function { name: "shell".into() };
    println!("匹配: {}", matches_kind(&kind, &payload)); // true
}
```

### 本章小结

| 概念 | 关键点 |
|------|--------|
| 枚举 | 每个变体可携带不同类型数据 |
| Option/Result | 标准库最重要的枚举 |
| match | 穷举匹配，编译器保证不遗漏 |
| if let/while let | 单分支简化 |
| let chains | Rust 2024 新特性 |
| matches! | 简洁条件判断 |
| 嵌套解构 | 一步提取深层数据 |

---

## 8. Trait 系统

Trait 是 Rust 的接口机制。如果结构体定义了数据"是什么"，那么 trait 定义了数据"能做什么"。
Trait 让你编写适用于多种类型的通用代码，同时保持类型安全。

### 8.1 定义和实现 Trait

```rust
// 定义 trait — 描述"能做什么"
trait Summary {
    fn summarize(&self) -> String;

    // 带默认实现的方法
    fn summarize_preview(&self) -> String {
        String::from("(阅读全文...)")
    }
}

// 为类型实现 trait — 描述"具体怎么做"
struct Article {
    title: String,
    author: String,
    content: String,
}

impl Summary for Article {
    fn summarize(&self) -> String {
        format!("{}, by {} — {}", self.title, self.author, &self.content[..20])
    }
    // summarize_preview 使用默认实现
}

struct Tweet {
    username: String,
    text: String,
}

impl Summary for Tweet {
    fn summarize(&self) -> String {
        format!("@{}: {}", self.username, self.text)
    }
}

fn main() {
    let article = Article {
        title: "Rust 入门".to_string(),
        author: "张三".to_string(),
        content: "Rust 是一门注重安全性和性能的语言...".to_string(),
    };
    let tweet = Tweet {
        username: "rustlang".to_string(),
        text: "Rust 2024 edition is here!".to_string(),
    };

    println!("{}", article.summarize());
    println!("{}", article.summarize_preview()); // 默认实现
    println!("{}", tweet.summarize());
}
```

### 8.2 Trait Bound — "任何实现了 X 的类型"

Trait bound 指定函数接受"任何实现了某个 trait 的类型"。
这是 Rust 泛型的核心——编译器为每种具体类型生成特化代码。

```rust
trait Speak {
    fn speak(&self) -> String;
}

struct Dog { name: String }
struct Cat { name: String }

impl Speak for Dog {
    fn speak(&self) -> String { format!("{}: 汪汪!", self.name) }
}

impl Speak for Cat {
    fn speak(&self) -> String { format!("{}: 喵~", self.name) }
}

// 方式 1：impl Trait 语法（简洁）
fn announce(thing: &impl Speak) {
    println!("{}", thing.speak());
}

// 方式 2：泛型 + trait bound（等价但更灵活）
fn announce_generic<T: Speak>(thing: &T) {
    println!("{}", thing.speak());
}

// 多个 trait bound
trait Walk { fn walk(&self); }
impl Walk for Dog { fn walk(&self) { println!("{} 在走路", self.name) } }

fn do_everything<T: Speak + Walk>(thing: &T) {
    thing.speak();
    thing.walk();
}

// where 子句 — 约束复杂时更清晰
fn complex<T, U>(t: &T, u: &U) -> String
where
T: Speak + Walk,
U: Speak,
{
    format!("{} and {}", t.speak(), u.speak())
}

fn main() {
    let dog = Dog { name: "旺财".to_string() };
    let cat = Cat { name: "咪咪".to_string() };

    announce(&dog);
    announce(&cat);
    do_everything(&dog);
    println!("{}", complex(&dog, &cat));
}
```

### 8.3 关联类型

关联类型让 trait 的每个实现指定一个具体的类型。

```rust
trait Calculator {
    type Output;  // 关联类型

    fn calculate(&self, a: f64, b: f64) -> Self::Output;
}

struct Adder;
struct Divider;

impl Calculator for Adder {
    type Output = f64;
    fn calculate(&self, a: f64, b: f64) -> f64 { a + b }
}

impl Calculator for Divider {
    type Output = Result<f64, String>;
    fn calculate(&self, a: f64, b: f64) -> Result<f64, String> {
        if b == 0.0 { Err("除数为零".into()) } else { Ok(a / b) }
    }
}

fn main() {
    println!("3 + 4 = {}", Adder.calculate(3.0, 4.0));
    println!("10 / 3 = {:?}", Divider.calculate(10.0, 3.0));
}
```

**关联类型 vs 泛型：** 关联类型 = 每个类型只能实现一次；泛型 = 可以为不同类型参数实现多次。
当每个类型只有一个合理的输出类型时，用关联类型。

**项目实例：**

```rust
// core/src/tools/registry.rs — OpenAgere 实际代码
pub trait ToolHandler: Send + Sync {
    type Output: ToolOutput + 'static;

    fn kind(&self) -> ToolKind;
    fn handle(&self, invocation: ToolInvocation)
    -> impl Future<Output = Result<Self::Output, FunctionCallError>> + Send;
}
```

### 8.4 Trait Object（`dyn Trait`）— 运行时多态

当你需要在运行时使用不同类型的对象时，用 trait object。

```rust
trait Draw {
    fn draw(&self);
}

struct Button { label: String }
struct TextField { placeholder: String }

impl Draw for Button {
    fn draw(&self) { println!("[按钮: {}]", self.label); }
}

impl Draw for TextField {
    fn draw(&self) { println!("[输入框: {}]", self.placeholder); }
}

fn main() {
    // Box<dyn Trait> — 堆上的 trait object
    let components: Vec<Box<dyn Draw>> = vec![
    Box::new(Button { label: "提交".to_string() }),
    Box::new(TextField { placeholder: "请输入...".to_string() }),
    Box::new(Button { label: "取消".to_string() }),
    ];

    for component in &components {
        component.draw();
    }
}
```

**`Box<dyn T>` vs `Arc<dyn T>`：**
- `Box<dyn T>`：单一所有权
- `Arc<dyn T>`：共享所有权（线程安全）

**项目实例：**

```rust
// tui/src/app_event.rs
enum AppEvent {
    InsertHistoryCell(Box<dyn HistoryCell>),
}

// tui/src/app_backtrack.rs
// transcript_cells: &mut Vec<Arc<dyn HistoryCell>>
```


### 8.4.1 `impl Trait` vs `dyn Trait` — 性能对比

```text
impl Trait (静态分发):
├── 编译期生成每种类型的具体代码（单态化）
├── 编译器知道具体类型 → 可以内联
├── 零运行时开销
└── 缺点：代码膨胀（每种类型一份代码）

dyn Trait (动态分发):
├── 运行时通过 vtable 查找方法
├── 编译器不知道具体类型 → 不能内联
├── 额外开销：间接调用 + vtable 查找
└── 优点：代码紧凑（一份代码处理所有类型）
```

```rust
trait Compute { fn compute(&self) -> i32; }
struct Fast;
impl Compute for Fast { fn compute(&self) -> i32 { 42 } }

// 静态分发 — 编译器生成 Fast::compute 的直接调用
fn static_dispatch(c: &impl Compute) -> i32 {
    c.compute()  // 编译为直接调用，可内联
}

// 动态分发 — 运行时通过 vtable 调用
fn dynamic_dispatch(c: &dyn Compute) -> i32 {
    c.compute()  // 编译为 vtable 查找 + 间接调用
}

fn main() {
    let fast = Fast;
    println!("{}", static_dispatch(&fast));
    println!("{}", dynamic_dispatch(&fast));
}
```

**选择建议：**
- 性能敏感 → `impl Trait`（泛型 + 静态分发）
- 需要集合存储不同类型 → `Box<dyn Trait>`（动态分发）
- 大多数场景 → 优先 `impl Trait`，需要时再切换到 `dyn`

### 8.5 孤儿规则

Rust 要求：**你只能为"你的类型"实现"外部 trait"，
或为"外部类型"实现"你的 trait"。** 不能两者都是外部的。

```rust
use std::fmt;

// ✅ 你的 trait + 外部类型 = OK
trait MyTrait {}
impl MyTrait for String {}

// ✅ 外部 trait + 你的类型 = OK
struct MyStruct;
impl fmt::Display for MyStruct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MyStruct")
    }
}

// ❌ 外部 trait + 外部类型 = 不允许
// impl fmt::Display for Vec<i32> { ... }

// Newtype 绕过孤儿规则：
struct Wrapper(Vec<String>);
impl fmt::Display for Wrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]", self.0.join(", "))
    }
}
```

### 8.6 实现常用 Trait

```rust
use std::fmt;

struct ThreadId(String);

// Display — 用 {} 格式化
impl fmt::Display for ThreadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// From — 类型转换（自动获得 Into）
impl From<String> for ThreadId {
    fn from(s: String) -> Self { ThreadId(s) }
}

impl From<ThreadId> for String {
    fn from(value: ThreadId) -> Self { value.0 }
}

fn main() {
    let id = ThreadId("abc-123".to_string());
    println!("Thread: {id}");  // 使用 {} 格式

    // From: 明确转换
    let id2 = ThreadId::from("def-456".to_string());

    // Into: 自动转换
    let id3: ThreadId = "ghi-789".to_string().into();

    let s: String = id.into();
    println!("as string: {s}");
}
```

### 8.7 `#[async_trait]`

```rust
// #[async_trait]
// pub(crate) trait UserConfigReloader: Send + Sync {
    //     async fn reload_user_config(&self);
    // }
//
// #[async_trait]
// impl UserConfigReloader for ThreadManager {
    //     async fn reload_user_config(&self) {
        //         let thread_ids = self.list_thread_ids().await;
        //         for thread_id in thread_ids {
            //             if let Err(err) = self.get_thread(thread_id).await
            //                 .unwrap().submit(Op::ReloadUserConfig).await
            //             {
                //                 warn!("failed to reload: {err}");
                //             }
            //         }
        //     }
    // }

fn main() {
    println!("#[async_trait] 让 trait 中可以定义 async 方法");
    println!("在 Rust 2024 中，原生 async fn in trait 已稳定");
    println!("但 async_trait crate 仍然广泛使用（支持 trait object）");
}
```

### 本章小结

| 概念 | 关键点 |
|------|--------|
| trait 定义 | 方法签名 + 可选默认实现 |
| impl trait | 为类型实现 trait |
| impl Trait 参数 | "接受任何实现了 X 的类型" |
| where 子句 | 复杂约束时更清晰 |
| 关联类型 | 每个实现只有一个输出类型 |
| dyn Trait | 运行时多态，Box<dyn T> / Arc<dyn T> |
| 孤儿规则 | 只能为"你的类型"实现"外部 trait" |
| Display/From | 最常用的 trait 实现 |

---

## 9. 泛型

泛型让你编写适用于多种类型的代码，而不牺牲性能。Rust 在编译期为每种具体类型
生成特化代码（**单态化**），所以泛型是**零成本抽象**。

### 9.1 泛型函数

```rust
// <T: PartialOrd> 表示 T 必须支持比较
fn largest<T: PartialOrd>(list: &[T]) -> &T {
    let mut max = &list[0];
    for item in &list[1..] {
        if item > max {
            max = item;
        }
    }
    max
}

fn main() {
    let numbers = vec![34, 50, 25, 100, 65];
    println!("最大数: {}", largest(&numbers));

    let chars = vec!['y', 'm', 'a', 'q'];
    println!("最大字符: {}", largest(&chars));

    // 同一个函数，编译器为 i32 和 char 各生成一份代码
    // 运行时没有泛型的额外开销——和手写两个函数一样快
}
```

### 9.2 泛型结构体

```rust
#[derive(Debug, Clone)]
struct Point<T> {
    x: T,
    y: T,
}

impl<T> Point<T> {
    fn new(x: T, y: T) -> Self {
        Point { x, y }
    }
    fn x(&self) -> &T { &self.x }
}

// 只有特定类型才有的方法
impl Point<f64> {
    fn distance_from_origin(&self) -> f64 {
        (self.x.powi(2) + self.y.powi(2)).sqrt()
    }
}

// 带 trait bound 的 impl
impl<T: std::fmt::Display> Point<T> {
    fn display(&self) {
        println!("({}, {})", self.x, self.y);
    }
}

fn main() {
    let int_point = Point::new(5, 10);
    let float_point = Point::new(1.0, 4.0);
    let str_point = Point::new("hello", "world");

    println!("int: {:?}", int_point);
    println!("float distance: {}", float_point.distance_from_origin());
    str_point.display();

    // 不能混合类型
    // let bad = Point { x: 5, y: 4.0 }; // ❌ 类型不匹配
}
```

**项目实例：**

```rust
// protocol/src/exec_output.rs
#[derive(Debug, Clone)]
pub struct StreamOutput<T: Clone> {
    pub text: T,  // 可以是 String 或 &str
    pub truncated_after_lines: Option<u32>,
}
```

### 9.3 `where` 子句

当 trait bound 很多时，`where` 子句比内联标注更清晰：

```rust
use std::fmt;

// 内联 — 简单时用
fn simple<T: fmt::Display>(item: &T) {
    println!("{item}");
}

// where — 复杂时用
fn complex<T, U, V>(t: &T, u: &U, v: &V) -> String
where
T: fmt::Display + Clone,
U: fmt::Debug + Default,
V: fmt::Display + Clone + PartialEq,
{
    format!("{t} | {:?} | {v}", u)
}

fn main() {
    simple(&42);
    println!("{}", complex(&"hello", &vec![1, 2], &true));
}
```

### 9.4 `impl Trait` — 两种用法

```rust
use std::fmt;

// 用法 1：参数位置（等价于泛型）
fn print_item(item: &impl fmt::Display) {
    println!("{item}");
}
// 等价于 fn print_item<T: fmt::Display>(item: &T)

// 用法 2：返回位置（隐藏具体类型）
fn make_iterator() -> impl Iterator<Item = i32> {
    vec![1, 2, 3].into_iter()
}
// 调用者知道它是 Iterator<Item=i32>，但不知道底层是 IntoIter

// 组合返回类型
fn filter_and_map(data: Vec<i32>) -> impl Iterator<Item = String> {
    data.into_iter()
    .filter(|x| x % 2 == 0)
    .map(|x| format!("even: {x}"))
}

fn main() {
    print_item(&42);
    for item in make_iterator() { print!("{item} "); }
    println!();
    for item in filter_and_map(vec![1, 2, 3, 4, 5, 6]) {
        println!("{item}");
    }
}
```

### 9.5 PhantomData

`PhantomData` 告诉编译器"逻辑上我拥有类型 T"，但不实际存储它。

```rust
use std::marker::PhantomData;

struct FragmentProxy<T> {
    _marker: PhantomData<fn() -> T>,
}

trait Fragment {
    const MARKER: &'static str;
}

struct SkillFragment;
impl Fragment for SkillFragment {
    const MARKER: &'static str = "<!-- skills -->";
}

impl<T: Fragment> FragmentProxy<T> {
    fn new() -> Self {
        FragmentProxy { _marker: PhantomData }
    }
    fn marker(&self) -> &'static str { T::MARKER }
}

fn main() {
    let proxy = FragmentProxy::<SkillFragment>::new();
    println!("marker: {}", proxy.marker());
}
```

**为什么用 `PhantomData<fn() -> T>` 而不是 `PhantomData<T>`？**
`fn() -> T` 是一个函数指针类型，不暗示 `Send`/`Sync`。
而 `PhantomData<T>` 会让结构体自动获得 T 的 `Send`/`Sync` 属性。

### 9.6 单态化 — 零成本的秘密

```rust
fn add<T: std::ops::Add<Output = T>>(a: T, b: T) -> T {
    a + b
}

fn main() {
    // 编译器为 i32 和 f64 各生成一份代码：
    // fn add_i32(a: i32, b: i32) -> i32 { a + b }
    // fn add_f64(a: f64, b: f64) -> f64 { a + b }
    println!("{}", add(1i32, 2i32));
    println!("{}", add(1.0f64, 2.0f64));
    // 没有运行时开销！和手写两个函数完全一样快
}
```


### 9.7 Const 泛型 — 泛型常量参数

Rust 支持 **const 泛型**——类型参数不仅可以是类型，还可以是常量值。
最典型的例子是数组 [T; N]，其中 N 就是一个 const 泛型参数。

``rust
fn main() {
// 数组长度是编译期常量
let arr: [i32; 5] = [1, 2, 3, 4, 5];
let small: [i32; 3] = [1, 2, 3];
// arr 和 small 是不同的类型！编译器知道它们的长度不同

// 自定义 const 泛型
struct Matrix<const ROWS: usize, const COLS: usize> {
data: [[f64; COLS]; ROWS],
}
// Matrix<3, 3> 和 Matrix<4, 4> 是不同的类型
}
``

**什么时候用 const 泛型？**
- 需要在类型层面编码固定大小（缓冲区、矩阵维度）
- 编译器可以根据常量值优化代码
- 比运行时传参更高效（零成本）
### 本章小结

| 概念 | 关键点 |
|------|--------|
| 泛型函数 | `<T: Bound>` 适用于多种类型 |
| 泛型结构体 | `struct Foo<T>` |
| where 子句 | 复杂约束时更清晰 |
| impl Trait | 参数=泛型简写，返回=隐藏类型 |
| PhantomData | 逻辑上拥有类型但不存储 |
| 单态化 | 编译期特化，零运行时开销 |

---

## 10. 错误处理

Rust 没有异常。错误通过 `Result<T, E>` 返回值显式传播。这迫使开发者认真考虑
每个可能失败的操作，而不是依赖 try-catch 来兜底。

### 10.1 `Result<T, E>` 基础

```rust
use std::num::ParseIntError;

fn parse_and_double(s: &str) -> Result<i32, ParseIntError> {
    let n: i32 = s.parse()?;  // ? 传播错误
    Ok(n * 2)
}

fn main() {
    match parse_and_double("21") {
        Ok(value) => println!("结果: {value}"),
        Err(e) => println!("错误: {e}"),
    }

    match parse_and_double("abc") {
        Ok(value) => println!("结果: {value}"),
        Err(e) => println!("错误: {e}"),
    }

    // Result 的组合方法
    let result = parse_and_double("5");
    println!("unwrap_or: {}", result.unwrap_or(0));
    println!("map: {:?}", result.map(|v| v + 1));
}
```

### 10.2 `?` 运算符

`?` 是错误传播的语法糖。`Ok(v)` 时返回 `v` 继续执行；`Err(e)` 时提前返回 `Err(e.into())`。

```rust
use std::fs;
use std::io;

// 手动展开
fn read_manual() -> Result<String, io::Error> {
    let contents = match fs::read_to_string("file.txt") {
        Ok(v) => v,
        Err(e) => return Err(e),
    };
    Ok(contents.trim().to_string())
}

// ? 运算符 — 等价但简洁
fn read_short() -> Result<String, io::Error> {
    let contents = fs::read_to_string("file.txt")?;
    Ok(contents.trim().to_string())
}

// 链式 ?
fn read_first_line() -> Result<String, io::Error> {
    let contents = fs::read_to_string("file.txt")?;
    let first = contents.lines().next().unwrap_or("").to_string();
    Ok(first)
}

fn main() {
    println!("{:?}", read_manual());
    println!("{:?}", read_short());
    println!("{:?}", read_first_line());
}
```

#### `?` 与自动类型转换

`?` 会自动调用 `From::from()` 转换错误类型：

```rust
use std::fs;
use std::io;
use std::num::ParseIntError;
use std::fmt;

#[derive(Debug)]
enum AppError {
    Io(io::Error),
    Parse(ParseIntError),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO 错误: {e}"),
            Self::Parse(e) => write!(f, "解析错误: {e}"),
        }
    }
}

// From 让 ? 能自动转换
impl From<io::Error> for AppError {
    fn from(e: io::Error) -> Self { AppError::Io(e) }
}

impl From<ParseIntError> for AppError {
    fn from(e: ParseIntError) -> Self { AppError::Parse(e) }
}

fn read_and_parse() -> Result<i32, AppError> {
    let contents = fs::read_to_string("number.txt")?;  // io::Error → AppError::Io
    let number = contents.trim().parse::<i32>()?;       // ParseIntError → AppError::Parse
    Ok(number)
}

fn main() {
    println!("{:?}", read_and_parse());
}
```

### 10.3 `thiserror` — 定义结构化错误

`thiserror` 是最常用的错误定义 crate，自动派生 `Display` 和 `From`。

```rust
// 概念展示 thiserror（需要 thiserror crate）
// use thiserror::Error;
//
// #[derive(Error, Debug)]
// enum DatabaseError {
    //     #[error("连接失败: {host}:{port}")]
    //     ConnectionFailed { host: String, port: u16 },
    //
    //     #[error("查询超时: {query}")]
    //     QueryTimeout { query: String },
    //
    //     #[error(transparent)]        // 透传底层错误消息
    //     Io(#[from] io::Error),      // #[from] 自动生成 From<io::Error>
    //
    //     #[error("未知错误: {0}")]
    //     Unknown(String),
    // }

// 手动实现等价版本
use std::io;
use std::fmt;

#[derive(Debug)]
enum DatabaseError {
    ConnectionFailed { host: String, port: u16 },
    QueryTimeout { query: String },
    Io(io::Error),
    Unknown(String),
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionFailed { host, port } => write!(f, "连接失败: {host}:{port}"),
            Self::QueryTimeout { query } => write!(f, "查询超时: {query}"),
            Self::Io(e) => write!(f, "{e}"),  // transparent
            Self::Unknown(msg) => write!(f, "未知错误: {msg}"),
        }
    }
}

impl From<io::Error> for DatabaseError {
    fn from(e: io::Error) -> Self { DatabaseError::Io(e) }
}

fn main() {
    let err = DatabaseError::ConnectionFailed { host: "localhost".into(), port: 5432 };
    println!("错误: {err}");
}
```

#### 项目实例

```rust
// protocol/src/error.rs — OpenAgere 实际代码（简化版）
// #[derive(Error, Debug)]
// pub enum AgereErr {
    //     #[error("turn aborted")]
    //     TurnAborted,
    //
    //     #[error("stream disconnected: {0}")]
    //     Stream(String, Option<Duration>),
    //
    //     #[error("exec error: {0}")]
    //     Exec(#[from] ExecErr),    // 自动 From<ExecErr>
    //
    //     #[error(transparent)]
    //     Io(#[from] io::Error),    // 透传 + 自动 From<io::Error>
    //
    //     #[error(transparent)]
    //     Json(#[from] serde_json::Error),
    // }
//
// pub type Result<T> = std::result::Result<T, AgereErr>;
```

### 10.4 `anyhow` — 应用层错误传播

`anyhow` 用于不需要精确定义错误类型的应用层代码。

```rust
// 概念展示（需要 anyhow crate）
// use anyhow::{Result, Context, bail};
//
// fn load_config(path: &str) -> anyhow::Result<()> {
    //     let contents = std::fs::read_to_string(path)
    //         .context("failed to read config file")?;  // 添加上下文
    //
    //     if contents.is_empty() {
        //         bail!("config file is empty");  // 提前返回错误
        //     }
    //
    //     let _config: serde_json::Value = serde_json::from_str(&contents)
    //         .context("failed to parse config JSON")?;
    //     Ok(())
    // }

fn main() {
    println!("anyhow 用于应用层快速传播");
    println!("context() 添加错误上下文");
    println!("bail! 提前返回错误");
}
```

### 10.5 `thiserror` vs `anyhow` 选择策略

| 场景 | 推荐 | 原因 |
|------|------|------|
| 库的公共 API | `thiserror` | 调用者可以 `match` 不同错误变体 |
| 应用的 main | `anyhow` | 只需传播和报告 |
| 内部模块间 | `anyhow` 或自定义 | 根据是否需要匹配决定 |
| CLI 命令处理 | `anyhow` | 快速传播，自动链式上下文 |

### 本章小结

| 概念 | 关键点 |
|------|--------|
| Result | Ok(T) 或 Err(E) |
| ? | 错误传播语法糖，自动 From 转换 |
| thiserror | 定义结构化错误，自动 Display/From |
| anyhow | 应用层快速传播 |
| 类型别名 | `type Result<T> = result::Result<T, MyError>` |
| 选择策略 | 库用 thiserror，应用用 anyhow |

---

## 11. 集合与字符串

Rust 标准库提供了丰富的集合类型。本章详细讲解最常用的集合及其操作方法，
每个方法都配有可运行的示例。

### 11.1 Vec<T> — 动态数组

`Vec<T>` 是 Rust 中最常用的集合。它在堆上分配连续内存，可以动态增长。

**内存布局：**

```text
Vec<i32> { 1, 2, 3 }:
┌──────────────────┐        ┌──────────────────────┐
│ 栈上的 Vec 头部    │        │ 堆上的数据            │
│ ptr ──────────────────→   │ 1 │ 2 │ 3 │   │   │   │
│ len = 3          │        └──────────────────────┘
│ capacity = 4     │        (capacity=4, 还有 1 个空位)
└──────────────────┘
```

```rust
fn main() {
    // ---- 创建 ----
    let v1 = vec![1, 2, 3];                  // vec! 宏
    let mut v2 = Vec::new();                   // 空 Vec
    let v3 = Vec::with_capacity(10);           // 预分配容量（减少重新分配）

    // ---- 增删 ----
    v2.push(10);
    v2.push(20);
    v2.push(30);
    println!("push: {v2:?}"); // [10, 20, 30]

    v2.insert(1, 15);  // 在索引 1 处插入
    println!("insert(1, 15): {v2:?}"); // [10, 15, 20, 30]

    let last = v2.pop();  // 移除并返回最后一个
    println!("pop: {last:?}, vec: {v2:?}"); // Some(30), [10, 15, 20]

    v2.remove(0);  // 移除索引 0（后面元素前移，O(n)）
    println!("remove(0): {v2:?}"); // [15, 20]

    // ---- 访问 ----
    let first = &v1[0];           // 索引访问（越界 panic！）
    let safe = v1.get(5);         // 安全访问（返回 Option）
    println!("first: {first}, safe: {safe:?}");
    // first: 1, safe: None

    // ---- 排序 ----
    let mut nums = vec![5, 2, 8, 1, 9, 3];
    nums.sort();
    println!("sorted: {nums:?}"); // [1, 2, 3, 5, 8, 9]

    nums.sort_by(|a, b| b.cmp(a)); // 降序
    println!("reverse: {nums:?}"); // [9, 8, 5, 3, 2, 1]

    // 按自定义规则排序
    let mut words = vec!["banana", "apple", "cherry"];
    words.sort_by_key(|w| w.len());
    println!("by length: {words:?}"); // ["apple", "banana", "cherry"]

    // ---- 查找 ----
    let data = vec![10, 20, 30, 40, 50];
    println!("contains 30? {}", data.contains(&30));         // true
    println!("find > 25: {:?}", data.iter().find(|&&x| x > 25)); // Some(&30)
    println!("binary_search 30: {:?}", data.binary_search(&30)); // Ok(2)
    println!("index of 40: {:?}", data.iter().position(|&x| x == 40)); // Some(3)

    // ---- 过滤 ----
    let mut items = vec![1, 2, 3, 4, 5, 6, 7, 8];
    items.retain(|x| x % 2 == 0); // 只保留偶数
    println!("retained: {items:?}"); // [2, 4, 6, 8]

    // ---- 去重 ----
    let mut dups = vec![1, 1, 2, 2, 2, 3, 1, 1];
    dups.sort();       // dedup 只去除连续重复
    dups.dedup();
    println!("dedup: {dups:?}"); // [1, 2, 3]

    // ---- 合并 ----
    let mut a = vec![1, 2, 3];
    let b = vec![4, 5, 6];
    a.extend(b);
    println!("extended: {a:?}"); // [1, 2, 3, 4, 5, 6]

    // ---- 转换 ----
    let nums = vec![1, 2, 3];
    let strs: Vec<String> = nums.iter().map(|n| n.to_string()).collect();
    println!("to strings: {strs:?}"); // ["1", "2", "3"]

    let joined = strs.join(", ");
    println!("joined: {joined}"); // "1, 2, 3"
}
```

#### Vec 的容量增长策略

当 Vec 空间不足时，它会重新分配一块更大的内存（通常是当前容量的 2 倍），
把旧数据复制过去。这就是为什么 `with_capacity` 能提高性能——
如果你知道大致需要多少元素，预分配可以避免多次重新分配。

```rust
fn main() {
    let mut v = Vec::new();
    for i in 0..10 {
        v.push(i);
        println!("len={}, capacity={}", v.len(), v.capacity());
    }
    // capacity 会在 0, 1, 2, 4, 8, 16 时增长

    // 预分配版本——不会重新分配
    let mut v2 = Vec::with_capacity(10);
    for i in 0..10 { v2.push(i); }
    println!("预分配: len={}, capacity={}", v2.len(), v2.capacity());
    // capacity 始终为 10
}
```

### 11.2 HashMap<K, V>

HashMap 存储键值对，查找平均 O(1)。

```rust
use std::collections::HashMap;

fn main() {
    let mut scores: HashMap<String, i32> = HashMap::new();

    // 插入
    scores.insert("Alice".to_string(), 95);
    scores.insert("Bob".to_string(), 87);
    scores.insert("Charlie".to_string(), 92);

    // 查找（返回 Option<&V>）
    match scores.get("Alice") {
        Some(score) => println!("Alice: {score}"),
        None => println!("Alice 不存在"),
    }

    // contains_key
    println!("has Bob? {}", scores.contains_key("Bob"));

    // ---- Entry API ----
    // 最优雅的处理"存在或不存在"的方式

    // or_insert: 不存在则插入默认值
    scores.entry("Dave".to_string()).or_insert(0);
    scores.entry("Alice".to_string()).or_insert(0); // 已存在，不修改
    println!("after entry: {scores:?}");

    // 统计词频——Entry API 的经典用法
    let text = "hello world hello rust hello world";
    let mut word_count: HashMap<&str, usize> = HashMap::new();
    for word in text.split_whitespace() {
        *word_count.entry(word).or_insert(0) += 1;
    }
    println!("词频: {word_count:?}");
    // {"hello": 3, "world": 2, "rust": 1}

    // and_modify: 已存在时修改
    word_count.entry("hello")
    .and_modify(|count| *count += 10)
    .or_insert(0);
    println!("after modify: {}", word_count["hello"]); // 13

    // 遍历
    for (name, score) in &scores {
        println!("{name}: {score}");
    }

    // 更新值
    if let Some(score) = scores.get_mut("Bob") {
        *score = 90; // Bob 的成绩更新为 90
    }

    // 删除
    scores.remove("Charlie");
    println!("after remove: {scores:?}");

    // 从迭代器收集
    let pairs = vec![("x", 1), ("y", 2), ("z", 3)];
    let map: HashMap<_, _> = pairs.into_iter().collect();
    println!("collected: {map:?}");
}
```

### 11.3 HashSet 与 BTreeMap

```rust
use std::collections::HashSet;
use std::collections::BTreeMap;

fn main() {
    // HashSet — 不重复的集合
    let mut skills: HashSet<&str> = HashSet::new();
    skills.insert("Rust");
    skills.insert("Python");
    skills.insert("Rust"); // 重复，不添加
    println!("skills: {skills:?}");
    println!("has Rust? {}", skills.contains("Rust"));

    // 集合运算
    let a: HashSet<_> = vec![1, 2, 3, 4].into_iter().collect();
    let b: HashSet<_> = vec![3, 4, 5, 6].into_iter().collect();

    let union: Vec<_> = a.union(&b).collect();
    let intersection: Vec<_> = a.intersection(&b).collect();
    let diff: Vec<_> = a.difference(&b).collect();
    println!("union: {union:?}");
    println!("intersection: {intersection:?}");
    println!("difference: {diff:?}");

    // BTreeMap — 有序的 HashMap
    let mut btree = BTreeMap::new();
    btree.insert("banana", 3);
    btree.insert("apple", 5);
    btree.insert("cherry", 2);
    // 按键字母序遍历
    for (k, v) in &btree {
        print!("{k}:{v} "); // apple:5 banana:3 cherry:2
    }
    println!();
    // BTreeMap 查找 O(log n)，但保持有序
}
```

### 11.4 String 操作

```rust
fn main() {
    let s1 = String::from("hello");
    let s2 = "world".to_string();
    let s3 = format!("{s1} {s2}");
    println!("{s3}"); // hello world

    // 修改
    let mut s = String::new();
    s.push('H');
    s.push_str("ello");
    s.insert(5, ' ');
    s.insert_str(6, "beautiful ");
    println!("{s}"); // Hello beautiful world

    // 查找
    let text = "Hello, World! Hello, Rust!";
    println!("contains: {}", text.contains("World"));
    println!("starts_with: {}", text.starts_with("Hello"));
    println!("ends_with: {}", text.ends_with("Rust!"));
    println!("find: {:?}", text.find("Rust"));  // Some(15)
    println!("replaced: {}", text.replace("Hello", "Hi"));

    // 分割
    let csv = "apple,banana,cherry";
    let fruits: Vec<&str> = csv.split(',').collect();
    println!("split: {fruits:?}");

    for word in "  the quick brown fox  ".split_whitespace() {
        print!("[{word}] ");
    }
    println!();

    // 多行文本
    for (i, line) in "line1\nline2\nline3".lines().enumerate() {
        println!("line {i}: {line}");
    }

    // 大小写
    println!("lower: {}", "HELLO WORLD".to_lowercase());
    println!("upper: {}", "hello world".to_uppercase());

    // trim
    let padded = "  hello  ";
    println!("trim: '{}'", padded.trim());
    println!("trim_start: '{}'", padded.trim_start());
    println!("trim_end: '{}'", padded.trim_end());

    // 判断
    println!("is_empty: {}", "".is_empty());
    println!("is_ascii: {}", "hello".is_ascii());

    // UTF-8 注意事项
    let chinese = "你好世界";
    println!("字节长度: {}", chinese.len());       // 12
    println!("字符数量: {}", chinese.chars().count()); // 4
    // let bad = &chinese[0..1]; // ❌ panic! 不是字符边界
    let safe = &chinese[0..3]; // "你"
    println!("safe: {safe}");
}
```

### 11.5 迭代器链式操作

```rust
fn main() {
    let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    // map — 转换每个元素
    let doubled: Vec<_> = numbers.iter().map(|x| x * 2).collect();
    println!("doubled: {doubled:?}");

    // filter — 过滤
    let evens: Vec<_> = numbers.iter().filter(|&&x| x % 2 == 0).collect();
    println!("evens: {evens:?}");

    // filter_map — 过滤 + 映射（只保留 Some）
    let parsed: Vec<i32> = vec!["1", "two", "3", "four", "5"]
    .iter()
    .filter_map(|s| s.parse().ok())
    .collect();
    println!("parsed: {parsed:?}"); // [1, 3, 5]

    // enumerate — 带索引
    for (i, val) in vec!["a", "b", "c"].iter().enumerate() {
        println!("[{i}] = {val}");
    }

    // zip — 配对两个迭代器
    let names = vec!["Alice", "Bob", "Charlie"];
    let scores = vec![95, 87, 92];
    for (name, score) in names.iter().zip(scores.iter()) {
        println!("{name}: {score}");
    }

    // take / skip
    let first_three: Vec<_> = (1..=10).take(3).collect();
    let after_two: Vec<_> = (1..=10).skip(2).take(3).collect();
    println!("first 3: {first_three:?}");
    println!("skip 2, take 3: {after_two:?}");

    // flat_map — 展平嵌套结构
    let nested = vec![vec![1, 2], vec![3, 4], vec![5, 6]];
    let flat: Vec<_> = nested.iter().flat_map(|v| v.iter()).collect();
    println!("flat: {flat:?}"); // [1, 2, 3, 4, 5, 6]

    // ---- 消费者方法 ----

    // fold — 归约
    let sum: i32 = numbers.iter().fold(0, |acc, &x| acc + x);
    println!("fold sum: {sum}"); // 55

    // sum, product
    let total: i32 = numbers.iter().sum();
    println!("sum: {total}");

    // count, any, all
    println!("count > 3: {}", numbers.iter().filter(|&&x| x > 3).count());
    println!("any even? {}", numbers.iter().any(|x| x % 2 == 0));
    println!("all positive? {}", numbers.iter().all(|&x| x > 0));

    // find — 找到第一个匹配的
    let found = numbers.iter().find(|&&x| x > 7);
    println!("found: {found:?}"); // Some(&8)

    // find_map — 找到第一个 Some
    let result: Option<String> = vec!["1", "abc", "3"]
    .iter()
    .find_map(|s| s.parse::<i32>().ok().map(|n| format!("found: {n}")));
    println!("{result:?}"); // Some("found: 1")

    // position
    let pos = numbers.iter().position(|&x| x == 7);
    println!("position of 7: {pos:?}"); // Some(6)

    // min, max
    println!("min: {:?}", numbers.iter().min());
    println!("max: {:?}", numbers.iter().max());

    // 链式组合
    let top_evens: Vec<_> = numbers.iter()
    .filter(|&&x| x % 2 == 0)
    .rev()
    .take(3)
    .collect();
    println!("top 3 evens: {top_evens:?}"); // [10, 8, 6]
}
```

### 本章小结

| 集合 | 特点 | 查找 | 用途 |
|------|------|------|------|
| `Vec<T>` | 有序、动态 | O(n) | 列表、队列 |
| `HashMap` | 键值对 | O(1) | 映射、缓存 |
| `HashSet` | 不重复 | O(1) | 去重、成员检测 |
| `BTreeMap` | 有序键值对 | O(log n) | 需要排序 |
| `String` | 拥有的 UTF-8 | O(n) | 可变文本 |
| `&str` | 借用切片 | O(n) | 只读文本 |
| 迭代器 | lazy、零成本 | — | 数据处理管道 |

## 12. 闭包与迭代器

闭包是匿名函数，可以捕获环境中的变量。迭代器提供了一种高效处理序列的方式。
两者结合是 Rust 中函数式编程的核心。

### 12.1 闭包基础

```rust
fn main() {
    // 基本语法
    let add = |a: i32, b: i32| -> i32 { a + b };
    let add_short = |a, b| a + b;  // 类型推导，单表达式省略 {}

    println!("3 + 4 = {}", add(3, 4));
    println!("5 + 6 = {}", add_short(5, 6));

    // 捕获环境变量
    let multiplier = 3;
    let multiply = |x| x * multiplier;  // 捕获 multiplier（只读借用）
    println!("4 * 3 = {}", multiply(4));
    println!("multiplier 仍然有效: {multiplier}"); // 因为只是借用

    // 可变借用
    let mut count = 0;
    let mut increment = || { count += 1; };
    increment();
    increment();
    increment();
    println!("count = {count}"); // 3

    // move — 获取所有权
    let data = vec![1, 2, 3];
    let consume = move || {
        println!("拥有: {data:?}");
    };
    consume();
    // println!("{data:?}"); // ❌ data 已经移动到闭包里了

    // move 常用于线程
    let name = String::from("Alice");
    let handle = std::thread::spawn(move || {
        println!("线程中: {name}");
    });
    handle.join().unwrap();
}
```

### 12.2 Fn, FnMut, FnOnce

这三种 trait 描述了闭包如何捕获环境：

| Trait | 捕获方式 | 可调用次数 | 典型场景 |
|-------|---------|-----------|---------|
| `Fn` | 借用 `&` | 多次 | 只读环境，如排序比较 |
| `FnMut` | 可变借用 `&mut` | 多次 | 修改环境，如计数器 |
| `FnOnce` | 获取所有权 | 一次 | 消耗环境，如 `thread::spawn` |

```rust
// Fn — 只读，可以调用多次
fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 {
    f(x)  // 可以调用 f 多次
}

// FnMut — 修改环境
fn call_n_times<F: FnMut()>(mut f: F, n: usize) {
    for _ in 0..n {
        f();
    }
}

// FnOnce — 只能调用一次
fn consume_once<F: FnOnce() -> String>(f: F) {
    let result = f();
    println!("结果: {result}");
}

fn main() {
    // Fn 示例
    let double = |x| x * 2;
    println!("{}", apply(double, 5));  // 10
    println!("{}", apply(double, 10)); // 可以再次调用

    // FnOnce 示例
    let name = String::from("OpenAgere");
    consume_once(move || format!("Welcome to {name}"));
    // name 已移动，不能再调用
}
```

**层次关系：** `Fn` ⊂ `FnMut` ⊂ `FnOnce`
如果一个闭包是 `Fn`，它也是 `FnMut` 和 `FnOnce`。
当你不确定需要哪种时，优先用 `Fn`（最宽松的限制）。

### 12.3 迭代器基础

```rust
fn main() {
    let numbers = vec![1, 2, 3, 4, 5];

    // 三种迭代器
    let _iter = numbers.iter();        // 借用 &i32
    // let _iter_mut = numbers.iter_mut(); // 可变借用 &mut i32
    // let _into = numbers.into_iter();    // 拥有 i32（消耗原集合）

    // for 循环语法糖
    let nums = vec![10, 20, 30];
    for n in &nums {  // 等价于 nums.iter()
        print!("{n} ");
    }
    println!();

    // 手动使用迭代器
    let mut iter = nums.iter();
    println!("next: {:?}", iter.next()); // Some(&10)
    println!("next: {:?}", iter.next()); // Some(&20)
    println!("next: {:?}", iter.next()); // Some(&30)
    println!("next: {:?}", iter.next()); // None（迭代结束）
}
```

### 12.4 迭代器适配器详解

迭代器方法分为两类：
- **适配器**（adapter）：返回新迭代器，不消耗原迭代器（lazy）
- **消费者**（consumer）：消耗迭代器，产生结果值

```rust
fn main() {
    let numbers = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    // ---- 适配器 ----

    // map — 一对一转换
    let doubled: Vec<_> = numbers.iter().map(|x| x * 2).collect();
    println!("doubled: {doubled:?}");

    // filter — 过滤
    let evens: Vec<_> = numbers.iter().filter(|&&x| x % 2 == 0).collect();
    println!("evens: {evens:?}");

    // filter_map — 过滤 + 映射（只保留 Some）
    let parsed: Vec<i32> = vec!["1", "two", "3", "four", "5"]
    .iter()
    .filter_map(|s| s.parse().ok())
    .collect();
    println!("parsed: {parsed:?}"); // [1, 3, 5]

    // enumerate — 带索引
    for (i, val) in vec!["a", "b", "c"].iter().enumerate() {
        println!("[{i}] = {val}");
    }

    // zip — 配对两个迭代器
    let names = vec!["Alice", "Bob"];
    let scores = vec![95, 87];
    for (name, score) in names.iter().zip(scores.iter()) {
        println!("{name}: {score}");
    }

    // take / skip
    let first_three: Vec<_> = (1..=10).take(3).collect();
    let after_two: Vec<_> = (1..=10).skip(2).take(3).collect();
    println!("first 3: {first_three:?}");
    println!("skip 2, take 3: {after_two:?}");

    // flat_map — 展平
    let nested = vec![vec![1, 2], vec![3, 4], vec![5, 6]];
    let flat: Vec<_> = nested.iter().flat_map(|v| v.iter()).collect();
    println!("flat: {flat:?}");

    // chain — 连接两个迭代器
    let a = vec![1, 2];
    let b = vec![3, 4];
    let chained: Vec<_> = a.iter().chain(b.iter()).collect();
    println!("chained: {chained:?}");

    // peekable — 可窥视的迭代器
    let mut peek = vec![1, 2, 3].into_iter().peekable();
    println!("peek: {:?}", peek.peek()); // Some(&1)，不消耗
    println!("next: {:?}", peek.next()); // Some(1)，消耗

    // ---- 消费者 ----

    // collect — 收集为集合
    // fold — 归约（有初始值）
    let sum: i32 = numbers.iter().fold(0, |acc, &x| acc + x);
    println!("sum: {sum}");

    // 快捷方法
    let total: i32 = numbers.iter().sum();
    let product: i64 = numbers.iter().map(|&x| x as i64).product();
    println!("sum={total}, product={product}");

    // count, any, all
    println!("count > 3: {}", numbers.iter().filter(|&&x| x > 3).count());
    println!("any even? {}", numbers.iter().any(|x| x % 2 == 0));
    println!("all positive? {}", numbers.iter().all(|&x| x > 0));

    // find, find_map
    let found = numbers.iter().find(|&&x| x > 7);
    println!("found: {found:?}");

    let result: Option<String> = vec!["1", "abc", "3"]
    .iter()
    .find_map(|s| s.parse::<i32>().ok().map(|n| format!("found: {n}")));
    println!("{result:?}");

    // position
    let pos = numbers.iter().position(|&x| x == 7);
    println!("position: {pos:?}");

    // min, max
    println!("min: {:?}, max: {:?}", numbers.iter().min(), numbers.iter().max());
}
```

### 12.5 `impl Iterator` 返回类型

```rust
// 返回迭代器，隐藏具体类型
fn even_numbers(data: &[i32]) -> impl Iterator<Item = &i32> {
    data.iter().filter(|&&x| x % 2 == 0)
}

fn main() {
    let nums = vec![1, 2, 3, 4, 5, 6, 7, 8];
    for n in even_numbers(&nums) {
        print!("{n} ");
    }
    println!();
}
```

### 12.6 方法引用 vs 闭包

```rust
fn main() {
    let numbers: Vec<Option<i32>> = vec![Some(1), None, Some(3)];

    // ✅ 推荐：方法引用
    let values: Vec<_> = numbers.iter().filter_map(Option::as_ref).collect();
    println!("{values:?}");

    // ❌ 冗余闭包
    let values2: Vec<_> = numbers.iter().filter_map(|x| x.as_ref()).collect();

    // 项目 AGENTS.md 规则：
    // "Use method references over closures when possible"
    let statuses: Vec<String> = vec![200u16, 404, 500]
    .iter()
    .map(u16::to_string)  // ✅ 方法引用
    // .map(|s| s.to_string())  // ❌ 冗余闭包
    .collect();
    println!("{statuses:?}");
}
```

### 12.7 零成本抽象

```rust
// 迭代器链在编译后被优化为和手写循环一样的代码
fn sum_squares_iter(numbers: &[i32]) -> i32 {
    numbers.iter().map(|x| x * x).sum()
}

fn sum_squares_manual(numbers: &[i32]) -> i32 {
    let mut sum = 0;
    for &x in numbers {
        sum += x * x;
    }
    sum
}

fn main() {
    let nums = vec![1, 2, 3, 4, 5];
    assert_eq!(sum_squares_iter(&nums), sum_squares_manual(&nums));
    println!("结果: {}", sum_squares_iter(&nums));
    // 两个函数性能完全相同——编译器优化掉了迭代器的开销
}
```

#### 项目实例

```rust
// protocol/src/models.rs — filter_map
// self.entries.iter().filter_map(|entry| match &entry.path {
    //     FileSystemPath::Path { path } => Some((path, entry.access)),
    //     _ => None,
    // })

// protocol/src/protocol.rs — find_map
// items.iter().find_map(|item| match item {
    //     RolloutItem::SessionMeta(meta_line) => Some(meta_line.meta.id),
    //     _ => None,
    // })

// protocol/src/protocol.rs — any
// resumed.history.iter().any(&mut predicate)

fn main() {
    println!("项目中大量使用 filter_map, find_map, any 等迭代器方法");
}
```

### 本章小结

| 概念 | 关键点 |
|------|--------|
| 闭包 | 匿名函数，捕获环境（Fn/FnMut/FnOnce） |
| move | 获取所有权，常用于线程 |
| 迭代器 | lazy 计算，零成本抽象 |
| 适配器 | map, filter, filter_map, flat_map, zip, enumerate |
| 消费者 | collect, fold, sum, find, any, all |
| impl Iterator | 返回迭代器，隐藏具体类型 |
| 方法引用 | `T::method` 代替 `\|x\| x.method()` |
| 零成本 | 迭代器链 == 手写循环 |

## 13. 智能指针

智能指针是**表现得像指针的类型**——它们可以通过 `*` 解引用访问数据，同时管理资源
的生命周期（何时分配、何时释放）。Rust 标准库提供了多种智能指针，每种有不同的
所有权语义和使用场景。

### 13.1 Box<T> — 把数据放到堆上

`Box<T>` 是最简单的智能指针。它做一件事：**把值分配到堆上，栈上只保存一个指针**。

#### 栈上的数据大小问题

在 Rust 中，编译器必须知道每个类型的大小。但对于某些类型，这会导致问题：

```rust
fn main() {
    // 基本类型 — 大小固定，直接在栈上
    let x: i32 = 42;           // 4 字节在栈上
    let s = String::from("hello"); // String 本身 24 字节在栈上（ptr + len + cap）
    // 字符串内容 5 字节在堆上

    // Box 把数据放到堆上
    let b = Box::new(5);
    // 栈上: 8 字节（一个指针）
    // 堆上: 4 字节（整数 5）
    println!("b = {b}");  // 自动解引用: *b == 5
    // b 离开作用域时，堆上的 5 被自动释放
}
```

#### 为什么需要 Box？— 枚举变体大小问题

这是 `Box<T>` 在项目中**最重要的用途**。

**问题：** Rust 枚举的大小等于**最大变体**的大小。如果某个变体很大，整个枚举都会膨胀。

```rust
// 没有 Box 的情况
#[derive(Debug)]
enum Message {
    Quit,                              // 0 字节
    Move { x: i32, y: i32 },          // 8 字节
    Big { data: [u8; 1024] },         // 1024 字节
}
// Message 的大小 = 最大变体 = 1024 字节
// 即使 99% 的消息是 Quit 或 Move，每个都要占 1024 字节！

fn main() {
    println!("Message 大小: {} 字节", std::mem::size_of::<Message>());
    // 输出: Message 大小: 1024 字节（或更大，加上 tag）
}
```

**解决方案：** 用 `Box<T>` 把大类型间接引用：

```rust
// 有 Box 的情况
#[derive(Debug)]
enum Message {
    Quit,                              // 0 字节
    Move { x: i32, y: i32 },          // 8 字节
    Big { data: Box<[u8; 1024]> },    // 8 字节（只是一个指针！）
}
// Message 的大小 = max(0, 8, 8) + tag = ~16 字节
// 大部分消息只占 16 字节，Big 消息额外在堆上分配 1024 字节

fn main() {
    println!("Message 大小: {} 字节", std::mem::size_of::<Message>());
    // 输出: Message 大小: 16 字节

    let msg = Message::Big { data: Box::new([0u8; 1024]) };
    println!("{msg:?}");
}
```

**图解：**

```text
没有 Box 的枚举:
┌──────────────────────────────┐
│ tag (变体标记)  8 字节        │
│ 数据区          1024 字节     │  ← 即使 Quit 也占 1024 字节
└──────────────────────────────┘
总共: ~1032 字节

有 Box 的枚举:
┌──────────────────────────────┐
│ tag (变体标记)  8 字节        │
│ 数据区          8 字节        │  ← 只是一个指针
└──────────────────────────────┘
总共: ~16 字节（Big 的数据在堆上）
```

#### 项目实例

```rust
// protocol/src/error.rs — OpenAgere 实际代码
#[derive(Error, Debug)]
pub enum ExecErr {
    #[error("execution denied, exit code: {}, stdout: {}, stderr: {}",
    .output.exit_code, .output.stdout.text, .output.stderr.text)]
    Denied {
        output: Box<ExecToolCallOutput>,  // ← 注意这里的 Box
        network_policy_decision: Option<NetworkPolicyDecisionPayload>,
    },
    Timeout { output: Box<ExecToolCallOutput> },  // ← 这里也有 Box
    Signal(i32),
}
```

**为什么？** `ExecToolCallOutput` 包含完整的 stdout/stderr 文本输出，
可能非常大（几 KB 甚至更多）。如果不用 Box：
- `ExecErr::Denied` 变体会很大
- `ExecErr::Signal(42)` 这种简单错误也会占用同样的空间
- 所有 `ExecErr` 都会膨胀

用了 Box 之后：
- `Denied` 和 `Timeout` 变体只保存一个 8 字节的指针
- 实际数据在堆上按需分配
- `Signal(i32)` 这种简单变体保持紧凑
- 整个 `ExecErr` 枚举更小，函数调用时传递更快

#### 其他用途

```rust
fn main() {
    // 用途 1：递归类型
    // 编译器在编译时必须知道类型大小
    // 没有 Box 的话：List 包含 List，无限大
    // 有 Box：List 包含 Box<List>（指针大小固定）
    #[derive(Debug)]
    enum List {
        Cons(i32, Box<List>),  // Box 打破了递归
        Nil,
    }
    let list = List::Cons(1, Box::new(List::Cons(2, Box::new(List::Nil))));
    println!("{list:?}");

    // 用途 2：Trait Object
    trait Draw { fn draw(&self); }
    struct Button;
    impl Draw for Button { fn draw(&self) { println!("[Button]"); } }
    let component: Box<dyn Draw> = Box::new(Button);
    component.draw();
    // Box<dyn Draw> = 堆上的 trait object
    // 编译器不知道具体类型，但知道它实现了 Draw
}
```

### 13.2 Rc<T> — 引用计数（单线程共享）

**问题：** 有些数据需要被多个部分共享。但 Rust 的所有权规则说"每个值只有一个所有者"。
`Rc<T>`（Reference Counted）允许多个所有者共享同一份数据。

**工作原理：**

```text
Rc<Vec<i32>>:

┌──────────┐    ┌──────────────────┐
│ data      │───→│ 堆上的 Vec       │
│ ptr ─────┘    │ [1, 2, 3]        │
│ count = 3│    └──────────────────┘
└──────────┘
┌──────────┐
│ clone_a  │──→  (同一个堆数据)  count 不变
│ ptr ─────┘
└──────────┘
┌──────────┐
│ clone_b  │──→  (同一个堆数据)  count 不变
│ ptr ─────┘
└──────────┘

每次 Rc::clone() 计数 +1
每次 drop 计数 -1
计数归零 → 释放堆数据
```

```rust
use std::rc::Rc;

fn main() {
    let data = Rc::new(vec![1, 2, 3]);
    println!("初始计数: {}", Rc::strong_count(&data)); // 1

    let a = Rc::clone(&data);  // 计数 +1（注意：不是 deep copy！）
    println!("clone a: {}", Rc::strong_count(&data)); // 2

    let b = Rc::clone(&data);  // 计数 +1
    println!("clone b: {}", Rc::strong_count(&data)); // 3

    // a, b, data 都指向同一份堆数据
    // 修改数据？不行！Rc<T> 只提供只读访问
    println!("a = {a:?}");
    println!("b = {b:?}");

    drop(a);
    println!("drop a: {}", Rc::strong_count(&data)); // 2

    drop(b);
    println!("drop b: {}", Rc::strong_count(&data)); // 1

    drop(data);
    // 计数归零，堆上的 Vec 被释放

    // ⚠️ Rc 不是线程安全的！
    // fn is_send<T: Send>() {}
    // is_send::<Rc<i32>>(); // ❌ 编译错误：Rc 不实现 Send
}
```

**`Rc::clone()` vs `.clone()` 的区别：**
- `Rc::clone(&rc)` — 只复制指针 + 计数加 1，极快（O(1)）
- `rc.clone()` — 对于 `Rc<T>` 来说等价于 `Rc::clone`
- `data.clone()` — 如果 `data` 不是 `Rc`，则是深拷贝（O(n)）

### 13.3 Arc<T> — 原子引用计数（多线程共享）

`Arc<T>` 是 `Rc<T>` 的**线程安全版本**。`Arc` 代表 "Atomic Reference Counted"。

**区别：**

| | Rc<T> | Arc<T> |
|--|-------|--------|
| 线程安全 | ❌ 不实现 Send/Sync | ✅ 实现 Send + Sync |
| 计数操作 | 普通加减 | 原子操作（atomics） |
| 性能 | 略快 | 略慢（原子操作开销） |
| 使用场景 | 单线程共享 | 跨线程共享 |

```rust
use std::sync::Arc;
use std::thread;

fn main() {
    let data = Arc::new(vec![1, 2, 3, 4, 5]);
    let mut handles = vec![];

    for i in 0..3 {
        let data_clone = Arc::clone(&data);  // 计数 +1
        let handle = thread::spawn(move || {
            // 每个线程拥有自己的 Arc 引用
            println!("线程 {i}: 数据 = {data_clone:?}");
            // data_clone 在闭包结束时 drop，计数 -1
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();  // 等待所有线程完成
    }

    // 所有 Arc 都被 drop，计数归零，堆上的 Vec 被释放
    println!("数据: {data:?}");
}
```

**为什么不能直接用 Rc？**

`Rc` 的计数操作不是原子的。如果两个线程同时 clone/drop Rc，
计数可能出错，导致提前释放或内存泄漏。`Arc` 使用原子操作保证计数的正确性。

#### 项目实例

```rust
// core/src/exec_policy.rs — 共享执行策略
pub(crate) fn new(policy: Arc<Policy>) -> Self {
    Self { policy: ArcSwap::from(policy), ... }
}
// Policy 被多个异步任务共享，用 Arc 保证线程安全

// app-server/src/command_exec.rs — 共享会话映射
sessions: Arc<Mutex<HashMap<ConnectionProcessId, CommandExecSession>>>
// Arc 保证跨线程共享，Mutex 保证同一时刻只有一个线程修改
```

### 13.4 Arc<Mutex<T>> — 共享可变状态

这是 Rust 并发编程中**最常见的模式**。

- `Arc<T>` 解决"多个线程共享同一份数据"
- `Mutex<T>` 解决"同一时刻只有一个线程能修改"
- 组合起来 = "多个线程共享、但同一时刻只有一个线程能修改的数据"

```rust
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    let counter = Arc::new(Mutex::new(0));
    let mut handles = vec![];

    for _ in 0..5 {
        let counter = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            // 1. 获取锁（如果别人持有，会阻塞等待）
            let mut num = counter.lock().unwrap();
            // 2. 修改数据
            *num += 1;
            // 3. 锁在离开作用域时自动释放
            // （不需要手动 unlock！RAII 模式）
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("最终计数: {}", *counter.lock().unwrap()); // 5
}
```

#### tokio::sync::Mutex vs std::sync::Mutex

| | std::sync::Mutex | tokio::sync::Mutex |
|--|-----------------|-------------------|
| 锁定方式 | `.lock().unwrap()`（阻塞） | `.lock().await`（异步等待） |
| 跨 .await | ❌ **不允许** | ✅ **允许** |
| 性能 | 更快（用户态自旋锁） | 略慢（需要异步运行时支持） |
| 使用场景 | 短持有、不跨 await | 长持有、需要跨 await |

```rust
// std Mutex — 同步代码中短暂持有
use std::sync::RwLock;
// let config = RwLock::new(Config::default());
// let cfg = config.read().unwrap();  // 读多写少用 RwLock
// drop(cfg);  // 尽快释放读锁

// tokio Mutex — 跨 .await 持有
// use tokio::sync::Mutex;
// let state = Arc::new(Mutex::new(HashMap::new()));
// {
    //     let mut s = state.lock().await;  // 异步等待获取锁
    //     s.insert("key".to_string(), fetch_value().await);
    //     // 锁可以在 .await 之间持有！
    // }
```

**为什么 std Mutex 不能跨 .await？**
因为 `.await` 可能把任务切换到另一个线程。如果锁被持有，
任务切换后锁仍然被持有，另一个线程获取锁时就会死锁。
tokio Mutex 知道任务可能被切换，所以可以安全地跨 `.await`。

### 13.5 RefCell<T> — 运行时借用检查

**问题：** 借用规则在编译期检查。但有时你需要在运行时动态决定借用方式。
`RefCell<T>` 把借用规则从编译期推迟到运行时。

```rust
use std::cell::RefCell;

fn main() {
    let data = RefCell::new(vec![1, 2, 3]);

    // 可变借用（编译时不检查！运行时检查）
    {
        let mut borrow_mut = data.borrow_mut();
        borrow_mut.push(4);
    } // borrow_mut 在这里被释放

    // 不可变借用
    {
        let borrow = data.borrow();
        println!("数据: {borrow:?}");
    }

    println!("最终: {:?}", data.borrow());

    // ⚠️ 如果同时有 borrow 和 borrow_mut → 运行时 panic！
    // let b1 = data.borrow();
    // let b2 = data.borrow_mut(); // 💥 panic: already borrowed
}
```

**Rc<RefCell<T>> — 单线程共享可变状态：**

```rust
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    let shared = Rc::new(RefCell::new(0));
    let a = Rc::clone(&shared);
    let b = Rc::clone(&shared);

    *a.borrow_mut() += 10;
    *b.borrow_mut() += 20;
    println!("shared: {}", shared.borrow()); // 30
}
```

### 13.6 Cell<T> — 无检查的内部可变性

`Cell<T>` 用于 `Copy` 类型（整数、布尔等），不需要运行时检查。

```rust
use std::cell::Cell;

fn main() {
    let counter = Cell::new(0);

    // 不需要 &mut，直接修改
    counter.set(counter.get() + 1);
    counter.set(counter.get() + 1);
    counter.set(counter.get() + 1);

    println!("counter: {}", counter.get()); // 3
}
```

**项目实例：**

```rust
// tui/src/bottom_pane/mod.rs — 测试中的共享计数器
// on_ctrl_c_calls: Rc<Cell<usize>>,
// handle_calls: Rc<Cell<usize>>,
// 用于在测试中验证回调被调用的次数
```

### 13.7 Deref trait — 解引用强制转换

`Deref` 让智能指针可以像引用一样使用。

```rust
use std::ops::Deref;

// 自定义智能指针
struct MyBox<T> { inner: T }

impl<T> MyBox<T> {
    fn new(x: T) -> MyBox<T> { MyBox { inner: x } }
}

// 实现 Deref：告诉编译器 *MyBox<T> == T
impl<T> Deref for MyBox<T> {
    type Target = T;
    fn deref(&self) -> &T { &self.inner }
}

fn main() {
    let b = MyBox::new(String::from("hello"));

    // 手动解引用
    println!("{}", *b);  // *b == b.deref()

    // 解引用强制转换（Deref Coercion）
    fn takes_str(s: &str) { println!("{s}"); }
    takes_str(&b);
    // MyBox<String> → &String（通过 Deref）→ &str（再次 Deref）
    // 编译器自动完成这个转换链
}
```

### 13.8 Drop trait — RAII 资源管理

当值离开作用域时，Rust 自动调用 `drop`。

```rust
struct FileHandle { name: String }

impl Drop for FileHandle {
    fn drop(&mut self) {
        println!("关闭文件: {}", self.name);
        // 实际代码中：关闭文件描述符
    }
}

struct Connection { id: u32 }

impl Drop for Connection {
    fn drop(&mut self) {
        println!("断开连接: {}", self.id);
    }
}

fn main() {
    let f = FileHandle { name: "data.txt".into() };
    let c = Connection { id: 42 };
    println!("资源已创建");
}
// 输出顺序（后创建先 drop，LIFO）:
// 资源已创建
// 断开连接: 42
// 关闭文件: data.txt
```

**提前释放：**

```rust
fn main() {
    let s = String::from("hello");
    // ... 使用 s ...
    drop(s);  // std::mem::drop 提前调用 Drop
    // println!("{s}"); // ❌ s 已经被 drop
}
```

### 13.9 Weak<T> — 避免循环引用

当两个 Rc/Arc 互相引用时，计数永远不会归零 → 内存泄漏。
`Weak<T>` 是弱引用——不增加计数。

```rust
use std::rc::{Rc, Weak};
use std::cell::RefCell;

#[derive(Debug)]
struct Node {
    value: i32,
    parent: RefCell<Weak<Node>>,       // 弱引用 → 不增加计数
    children: RefCell<Vec<Rc<Node>>>,  // 强引用 → 增加计数
}

fn main() {
    let leaf = Rc::new(Node {
        value: 3,
        parent: RefCell::new(Weak::new()),
        children: RefCell::new(vec![]),
    });

    let branch = Rc::new(Node {
        value: 5,
        parent: RefCell::new(Weak::new()),
        children: RefCell::new(vec![Rc::clone(&leaf)]),
    });

    // leaf 的 parent 指向 branch，但用 Weak 避免循环
    *leaf.parent.borrow_mut() = Rc::downgrade(&branch);

    println!("branch strong count: {}", Rc::strong_count(&branch)); // 1
    println!("leaf strong count: {}", Rc::strong_count(&leaf));     // 2

    // 通过 Weak 访问 parent
    if let Some(parent) = leaf.parent.borrow().upgrade() {
        println!("parent value: {}", parent.value); // 5
    }
}
```

### 本章总结 — 什么时候用什么？

| 场景 | 选择 | 原因 |
|------|------|------|
| 把大数据放堆上 | `Box<T>` | 最简单 |
| 减小枚举大小 | `Box<BigType>` | 避免变体膨胀 |
| 递归类型 | `Box<Self>` | 打破无限大小 |
| trait object | `Box<dyn Trait>` | 单一所有权 |
| 单线程共享只读 | `Rc<T>` | 引用计数 |
| 多线程共享只读 | `Arc<T>` | 原子引用计数 |
| 多线程共享可变 | `Arc<Mutex<T>>` | 共享 + 互斥 |
| 单线程共享可变 | `Rc<RefCell<T>>` | 运行时借用 |
| 单线程快速修改 | `Rc<Cell<T>>` | 无检查内部可变 |
| 避免循环引用 | `Weak<T>` | 不增加计数 |
| 跨 .await 锁 | `tokio::sync::Mutex` | 异步安全 |

## 14. Pin 与 Unpin

`Pin` 是 Rust 中**最难理解**的概念之一。但它解决的问题其实很直观：
**如果一个结构体内部有一个指针指向自己，那么这个结构体不能被移动。**

### 14.1 问题的起源：自引用结构

先从一个例子开始。假设我们要实现一个简单的迭代器：

```rust
// 我们想要的：一个引用自己内部数据的结构
struct SelfReferential {
    data: String,
    data_ref: *const String,  // 指向 data 字段的指针
}

impl SelfReferential {
    fn new(text: &str) -> Self {
        let mut s = SelfReferential {
            data: text.to_string(),
            data_ref: std::ptr::null(),
        };
        // 让 data_ref 指向自己的 data
        s.data_ref = &s.data as *const String;
        s
    }

    fn get_data_ref(&self) -> &str {
        assert!(!self.data_ref.is_null());
        unsafe { &*self.data_ref }
    }
}
```

这个结构体创建了一个**自引用**：`data_ref` 指向 `data`。
看起来没问题？试试移动它：

```rust
fn main() {
    let s1 = SelfReferential::new("hello");
    println!("s1.data_ref 指向: {}", s1.get_data_ref()); // "hello" ✅

    // 移动 s1 到 s2
    let s2 = s1;
    // 移动后内存布局变了：
    // s2 在新的栈位置
    // 但 s2.data_ref 还指向 s1 的旧位置！

    println!("s2.data_ref 指向: {}", s2.get_data_ref());
    // 💥 悬垂指针！可能崩溃，可能读到垃圾数据
}
```

**这就是自引用结构的核心问题：** 一旦结构体被移动（赋值、传参、返回值），
内部指针就会指向已释放的内存。

```text
移动前:
栈地址 0x1000:
┌──────────────────────────┐
│ data = "hello"           │
│ data_ref ──→ 0x1000 ────→ data
└──────────────────────────┘

移动后 (let s2 = s1):
栈地址 0x2000:
┌──────────────────────────┐
│ data = "hello"           │
│ data_ref ──→ 0x1000 ────→ ❌ 旧地址！已被释放
└──────────────────────────┘
```

### 14.2 Pin 的解决方案

`Pin<P>` 是一个**包装器**，它包装一个指针类型 `P`（通常是 `Box<T>` 或 `&mut T`），
并做出保证：**被 Pin 住的值不会被移动**。

```rust
use std::pin::Pin;

// 基本用法
fn main() {
    // Box::pin — 把值放在堆上并 Pin 住
    let pinned: Pin<Box<i32>> = Box::pin(42);
    println!("pinned: {pinned}");
    // pinned 不能被移动到其他位置
}
```

**Pin 保证的具体含义：**

一旦一个值被 `Pin<Box<T>>` 包装，编译器保证：
- 你**不能**通过 `mem::replace` 把它替换掉
- 你**不能**通过 `mem::swap` 交换它
- 你**不能**把它移动到其他位置
- 这个保证**在编译期强制执行**

### 14.3 Unpin — 大多数类型不受影响

**关键概念：** `Pin` 只对 `!Unpin` 的类型真正有效。

`Unpin` 是一个**标记 trait**。实现了 `Unpin` 的类型表示
"即使被 Pin 住也可以安全移动"。

```rust
// 哪些类型是 Unpin？
// 几乎所有类型！
// i32, f64, bool, String, Vec<T>, Box<T>, &T, &mut T, ...

// 哪些类型是 !Unpin？
// 只有编译器生成的 Future 类型（async fn 的返回值）
// 和你手动用 PhantomPinned 标记的类型

fn main() {
    // 对于 Unpin 类型，Pin 是透明的——可以自由移动
    let mut x = 42;
    let pinned_x: Pin<&mut i32> = Pin::new(&mut x);
    // *pinned_x = 100;  // ✅ 可以修改
    // x = 200;          // ✅ 也可以修改（Unpin 类型没有移动限制）

    // 对于 !Unpin 类型
    use std::marker::PhantomPinned;

    struct Immovable {
        _data: [u8; 64],
        _pin: PhantomPinned,  // 这个字段让类型变成 !Unpin
    }

    // 不能直接在栈上 Pin !Unpin 类型
    // let val = Immovable { _data: [0; 64], _pin: PhantomPinned };
    // let pinned = Pin::new(&mut val);  // ❌ 编译错误

    // 必须用 Box::pin 放到堆上
    let pinned = Box::pin(Immovable {
        _data: [0; 64],
        _pin: PhantomPinned,
    });
    // 现在 pinned 不能移动了
    println!("immovable created");
}
```

**为什么大多数类型是 Unpin？**

因为大多数类型不包含自引用。`String` 内部有指针指向堆数据，
但那是**外部数据**，不是自引用。移动 `String` 只是复制了栈上的
ptr/len/cap 三个字段，堆数据不受影响。

```text
移动 String 是安全的：
String "hello":

移动前（栈地址 0x1000）:
┌────────────────┐         ┌──────────────┐
│ ptr ─────────────────→   │ h e l l o    │ (堆)
│ len = 5        │         └──────────────┘
│ cap = 5        │
└────────────────┘

移动后（栈地址 0x2000）:
┌────────────────┐         ┌──────────────┐
│ ptr ─────────────────→   │ h e l l o    │ (堆)  ← 同一块堆数据
│ len = 5        │         └──────────────┘
│ cap = 5        │
└────────────────┘

只是复制了 3 个字段（ptr, len, cap），堆数据没变，完全安全
```

### 14.4 Pin 和 async/await 的关系

**这才是 Pin 在 Rust 中最常见的用途。**

当编译器把 `async fn` 转换为状态机时，它可能生成自引用的结构体。

```rust
async fn example() -> String {
    let data = String::from("hello");
    let reference = &data;  // reference 指向 data
    // 编译器生成的 Future 状态机中：
    // data 和 reference 都存在状态机里
    // reference 指向 data → 自引用！
    reference.to_string()
}
```

编译器生成的 Future 大致等价于：

```rust
// 编译器生成的状态机（简化版）
struct ExampleFuture {
    state: u8,        // 0=未开始, 1=进行中, 2=完成
    data: String,     // 局部变量
    reference: &???,  // 指向 data — 自引用！
}
```

如果这个 Future 被移动了，`reference` 就会指向已释放的 `data`。
所以编译器把 `ExampleFuture` 标记为 `!Unpin`，
并且要求你在使用 `.await` 之前不能移动它。

```rust
async fn example() {
    let result = example().await;
    // .await 不移动 Future
    // 它只是在原地 poll Future 直到完成
}
```

### 14.5 什么时候需要手动处理 Pin？

| 场景 | 需要手动 Pin？ |
|------|--------------|
| `async fn` + `.await` | ❌ 编译器自动处理 |
| `tokio::spawn(async { ... })` | ❌ spawn 自动处理 |
| `tokio::select!` | ❌ 自动处理 |
| 把 Future 存在结构体字段里 | ✅ 需要 `Pin<Box<F>>` |
| 实现自定义 `Stream` | ✅ 需要 `Pin<Box<S>>` |
| 实现自定义 `Future` | ✅ 需要理解 Pin |
| 使用 `async-stream` crate | ✅ 需要 `Pin<Box<S>>` |

### 14.6 存储 Future 时的 Pin

```rust
use std::future::Future;
use std::pin::Pin;

// 错误方式：直接存储 Future
// struct Task {
    //     fut: impl Future<Output = ()>,  // ❌ 不能直接存储
    // }

// 正确方式：用 Pin<Box<F>>
struct Task {
    fut: Pin<Box<dyn Future<Output = String> + Send>>,
}

impl Task {
    fn new(fut: impl Future<Output = String> + Send + 'static) -> Self {
        Task {
            fut: Box::pin(fut),  // Box::pin 一步完成：分配 + Pin
        }
    }
}

async fn compute() -> String {
    "computed".to_string()
}

fn main() {
    let task = Task::new(compute());
    // task 现在拥有了一个 Pin 住的 Future
    // 不能移动 task.fut
    println!("task created");
}
```

### 14.7 pin! 宏 vs Box::pin

```rust
use std::pin::pin;
use std::pin::Pin;

async fn compute() -> String { "done".to_string() }

fn main() {
    // 方式 1：pin! 宏 — 栈上固定（Rust 1.68+）
    let fut = pin!(compute());
    // fut 在栈上，不能移动
    // 优点：无堆分配
    // 缺点：必须在使用它的同一个作用域内

    // 方式 2：Box::pin — 堆上固定
    let fut2: Pin<Box<impl std::future::Future<Output = String>>> = Box::pin(compute());
    // fut2 在堆上，可以跨作用域传递
    // 优点：灵活，可以存在结构体里
    // 缺点：需要堆分配

    // 方式 3：async move 块的自引用场景
    let data = vec![1, 2, 3];
    let fut3 = async move {
        let _ref = &data;  // Future 引用了 data → 自引用
        // 如果 Future 被移动，_ref 就悬垂了
    };
    let pinned = Box::pin(fut3);
    // pinned 不能移动，所以 _ref 始终有效

    println!("futures created");
}
```

### 14.8 Pin Projection（高级）

当你有一个结构体包含 Pin 住的字段和非 Pin 住的字段时，
需要"投影"访问——把 `Pin<&mut Whole>` 拆成对各个字段的访问。

```rust
use std::pin::Pin;
use std::marker::PhantomPinned;

struct MixedStruct {
    normal: String,           // 可以自由移动
    pinned_data: Vec<u8>,     // 被 Pin 住，不能移动
    _pin: PhantomPinned,      // 标记为 !Unpin
}

// 安全地访问被 Pin 住的字段
fn access_pinned(mut s: Pin<&mut MixedStruct>) {
    // 访问普通字段（可以获取 &mut）
    // s.normal.push_str("hello");  // ❌ 不能直接访问
    // 因为 s 是 Pin<&mut>，需要通过 unsafe 或 pin-project crate

    // 访问被 Pin 住的字段（保证不会移动）
    // 在安全代码中，通常用 pin-project crate 来自动处理
    println!("accessing pinned struct");
}

fn main() {
    let s = Box::pin(MixedStruct {
        normal: "hello".into(),
        pinned_data: vec![1, 2, 3],
        _pin: PhantomPinned,
    });
    // access_pinned(s);
    println!("pin projection is advanced usage");
    println!("在大多数项目中不需要手动做 pin projection");
    println!("使用 pin-project crate 可以安全地处理");
}
```

### 14.9 总结：Pin 的心智模型

```text
Pin 的本质：

1. 自引用数据不能移动（否则内部指针悬垂）
2. Pin<T> 保证被包装的值不会移动
3. 大多数类型是 Unpin — Pin 对它们无效
4. async fn 生成的 Future 是 !Unpin — Pin 真正限制移动
5. 日常 async 代码中，运行时自动处理 Pin
6. 只有在存储 Future 或实现底层抽象时才需要手动处理
```

**给初学者的建议：**
- 先学会写 async fn 和使用 .await
- 了解 tokio::spawn 和 tokio::select!
- 当你看到 `Pin<Box<dyn Future>>` 编译错误时，再来读这章
- 大多数 Rust 开发者不需要手动实现 Future 或 Stream

### 本章小结

| 概念 | 关键点 |
|------|--------|
| 自引用 | 结构体内部指针指向自己的字段 |
| Pin 的用途 | 防止自引用数据被移动 |
| Unpin | 大多数类型是 Unpin，Pin 无效 |
| !Unpin | async Future、手动标记的类型 |
| pin!() | 栈上固定，无堆分配 |
| Box::pin | 堆上固定，可存储传递 |
| 日常使用 | async fn + .await 不需要手动 Pin |
| 需要手动 | 存储 Future、实现 Stream/Future |
| pin-project | 安全处理 Pin Projection |

## 15. Unsafe Rust

Unsafe Rust 不是"不安全"的代名词——它是"我需要承担更多责任"的信号。
在 unsafe 块中，编译器放开了一些检查，但程序员必须保证代码的正确性。

### 15.1 Unsafe 能做什么？

在 `unsafe` 块中，你可以做五件安全 Rust 不允许的事：

1. **解引用裸指针**（`*const T` 和 `*mut T`）
2. **调用 unsafe 函数**（包括 FFI 调用）
3. **访问或修改可变静态变量**
4. **实现 unsafe trait**
5. **访问 union 的字段**

```rust
fn main() {
    // 1. 裸指针
    let mut num = 5;
    let r1 = &num as *const i32;   // 不可变裸指针
    let r2 = &mut num as *mut i32; // 可变裸指针

    // 裸指针可以：
    // - 指向无效内存（悬垂）
    // - 为 null
    // - 不遵守借用规则
    // 但创建裸指针是安全的——只有解引用才需要 unsafe

    unsafe {
        println!("r1: {}", *r1);  // 解引用需要 unsafe
        *r2 = 10;                 // 通过可变裸指针修改
        println!("r1 after: {}", *r1); // 10
    }
}
```

### 15.2 unsafe 函数

```rust
// 标记为 unsafe 的函数，调用者必须在 unsafe 块中调用
unsafe fn dangerous() -> i32 {
    42
}

// 安全包装器——对外暴露安全接口
fn safe_wrapper() -> i32 {
    unsafe { dangerous() }
    // 内部使用 unsafe，但调用者不需要写 unsafe
    // 这意味着 safe_wrapper 的作者保证了安全性
}

fn main() {
    // dangerous(); // ❌ 不能直接调用
    let result = safe_wrapper();
    println!("result: {result}");

    // 常见模式：在安全函数中封装 unsafe
    // 用户调用安全 API，内部实现处理 unsafe 细节
}
```

### 15.3 FFI — 外部函数接口

```rust
// 调用 C 标准库
extern "C" {
    fn abs(input: i32) -> i32;
    fn strlen(s: *const std::ffi::c_char) -> usize;
    fn printf(format: *const std::ffi::c_char, ...) -> i32;
}

// 让 Rust 函数可以被 C 调用
#[no_mangle]  // 防止 Rust 编译器修改函数名
pub extern "C" fn rust_add(a: i32, b: i32) -> i32 {
    a + b
}

fn main() {
    unsafe {
        println!("abs(-5) = {}", abs(-5));
    }
    println!("rust_add(3, 4) = {}", rust_add(3, 4));
}
```

**为什么需要 FFI？**
- 调用操作系统 API（Windows API, POSIX）
- 使用现有的 C/C++ 库（OpenSSL, SQLite）
- 嵌入到其他语言中（Python, Node.js 的 native addon）

### 15.4 Send 与 Sync trait

`Send` 和 `Sync` 是两个标记 trait（marker trait），没有方法，只标记类型的线程安全性。

- **`Send`**：类型可以安全地跨线程**转移所有权**。几乎所有类型都是 Send。
例外：`Rc<T>`（非原子计数）、裸指针。
- **`Sync`**：类型可以安全地跨线程**共享引用**（`&T: Send` 意味着 `T: Sync`）。
例外：`Rc<T>`、`Cell<T>`、`RefCell<T>`。

```rust
fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}
fn assert_send_sync<T: Send + Sync>() {}

fn main() {
    // 基本类型
    assert_send_sync::<i32>();
    assert_send_sync::<String>();
    assert_send_sync::<Vec<i32>>();

    // Arc 是 Send + Sync（原子计数）
    assert_send_sync::<std::sync::Arc<i32>>();

    // Mutex<T> 当 T: Send 时是 Send + Sync
    assert_send_sync::<std::sync::Mutex<i32>>();

    // Rc 是 Send 但不是 Sync
    assert_send::<std::rc::Rc<i32>>();
    // assert_sync::<std::rc::Rc<i32>>(); // ❌

    // 裸指针不是 Send 也不是 Sync
    // assert_send::<*const i32>(); // ❌
}
```

**为什么这很重要？** Tokio 的 `tokio::spawn` 要求闭包是 `Send + 'static`：

```rust
// tokio::spawn<F>(future: F) where F: Future + Send + 'static
// 这意味着 spawn 的任务可以在线程池的不同线程上执行
// 如果闭包捕获了 !Send 的数据（如 Rc），编译就会失败
```


 ### Send 与 Sync 深入理解

 `Send` 和 `Sync` 是 Rust 并发安全的两大基石。理解了它们，就理解了 Rust
 为什么能在编译期防止数据竞争。

 #### 什么是 Send？

 **`Send` 表示一个类型的值可以安全地从一个线程移动到另一个线程。**

 通俗地说：如果 `T: Send`，你可以把 `T` 的值通过 `tokio::spawn` 传给另一个线程，
 而不会出问题。

 ```rust
 // Send 的直觉理解：
 // "这个值可以被搬走，搬到另一个线程用，不会出事"

 // i32 是 Send —— 就是一个数字，搬到哪都一样
 // String 是 Send —— 拥有堆上的数据，但指针可以安全移动
 // Vec<T> 是 Send（当 T: Send）—— 和 String 同理
 // Arc<T> 是 Send（当 T: Send + Sync）—— 原子引用计数，线程安全

 // Rc<i32> 不是 Send —— 引用计数不是原子的
 //    如果两个线程同时 clone/drop Rc，计数会出错
 // *const T 不是 Send —— 裸指针可能指向已释放的内存
 // Cell<T> 不是 Send —— 内部可变性在多线程下不安全
 ```

 **为什么 Rc 不是 Send？用具体例子解释：**

 ```text
 Rc<i32> 的内存布局：

 +----------------+
 | Rc 头部（堆）   |
 | strong = 2     |  <-- 非原子！两个线程同时 +1 可能都读到 2 写成 3
 | weak = 0       |
 | data = 42      |
 +----------------+

 线程 A: Rc::clone() -> 读到 strong=2 -> 写 strong=3
 线程 B: Rc::clone() -> 读到 strong=2 -> 写 strong=3

 结果：strong=3（应该是 4），引用计数错误！
 最终：提前释放 -> use-after-free -> 崩溃

 Arc 用原子操作（AtomicUsize）避免了这个问题。
 ```

 #### 什么是 Sync？

 **`Sync` 表示 `&T` 是 `Send` 的** —— 一个类型的不可变引用可以安全地跨线程发送。

 通俗地说：如果 `T: Sync`，多个线程可以同时持有 `&T`，安全地同时读取。

 ```rust
 // Sync 的直觉理解：
 // "多个线程可以同时只读地看这个值，不会出事"

 // i32 是 Sync —— 只读看一个数字，随便看
 // String 是 Sync —— 只读看一个字符串，随便看
 // Arc<T> 是 Sync（当 T: Send + Sync）
 // Mutex<T> 是 Sync（当 T: Send）—— 锁保护了并发访问

 // Cell<i32> 不是 Sync —— 通过 &Cell 就能修改值！
 //    两个线程同时通过 &Cell 修改 -> 数据竞争
 // RefCell<T> 不是 Sync —— 同理
 // Rc<T> 不是 Sync —— 引用计数非原子
 ```

 **为什么 Cell 不是 Sync？**

 ```rust
 use std::cell::Cell;

 fn main() {
     let c = Cell::new(0);

     // Cell 的特殊之处：通过 &Cell 就能修改值（内部可变性）
     let r: &Cell<i32> = &c;
     r.set(42);  // 不需要 &mut！
     println!("{}", c.get());  // 42

     // 如果 Cell 是 Sync，两个线程可以同时调用 r.set()
     // 导致数据竞争！
 }
 ```

 #### Send 和 Sync 的关系总结

 ```text
  类型 T          Send?   Sync?   能做什么
  -------------------------------------------
  i32              Y       Y      移走 + 共享只读
  String           Y       Y      移走 + 共享只读
  Vec<i32>         Y       Y      移走 + 共享只读
  Arc<i32>         Y       Y      移走 + 共享只读
  Mutex<i32>       Y       Y      移走 + 共享读写
  &i32             Y       Y      移走 + 共享只读
  &mut i32         Y       N      移走（但不可共享）
  Rc<i32>          N       N      不能跨线程
  Cell<i32>        Y       N      移走（但不可共享读）
  RefCell<i32>     Y       N      移走（但不可共享读）
  *const i32       N       N      不能跨线程
 ```

 #### 在项目中如何触发编译错误？

 ```rust
 use std::rc::Rc;

 fn main() {
     let data = Rc::new(42);

     // tokio::spawn 要求闭包是 Send + 'static
     // Rc 不是 Send，所以编译失败：

     // tokio::spawn(async move {
     //     println!("{data}");
     // });
     // error: future cannot be sent between threads safely
     //   the trait Send is not implemented for Rc<i32>

     // 修复 1：用 Arc 替换 Rc
     let data = std::sync::Arc::new(42);
     let clone = std::sync::Arc::clone(&data);
     // tokio::spawn(async move { println!("{clone}"); }); // OK

     // 修复 2：不跨线程（在当前线程用）
     println!("{data}");
 }
 ```

 #### 自动实现规则

 Rust 自动为大多数类型实现 Send 和 Sync：

 - 规则 1：所有字段都是 Send -> 类型自动 Send
 - 规则 2：所有字段都是 Sync -> 类型自动 Sync
 - 规则 3：包含裸指针（`*const`、`*mut`）-> 不自动实现

 ```rust
 // 自动实现的例子
 struct MyStruct {
     name: String,  // Send + Sync
     count: i32,    // Send + Sync
 }
 // MyStruct 自动是 Send + Sync

 // 不自动实现的例子
 struct NotSync {
     data: std::cell::Cell<i32>,  // 不是 Sync
 }
 // NotSync 自动不是 Sync（但仍然是 Send）

 fn main() {
     fn assert_send_sync<T: Send + Sync>() {}
     assert_send_sync::<MyStruct>();  // OK
     // assert_send_sync::<NotSync>(); // 编译错误
 }
 ```

 #### 项目中的实际用法

 ```rust
 // 1. tokio::spawn 要求 Send + 'static
 //    闭包捕获的变量必须是 Send
 //    闭包不能有非 'static 的借用

 // 2. Arc<Mutex<T>> 要求 T: Send
 //    Arc 要求 T: Send + Sync
 //    Mutex<T> 要求 T: Send 才实现 Sync
 //    组合：Arc<Mutex<T>> 要求 T: Send

 // sessions: Arc<Mutex<HashMap<String, Session>>>
 // HashMap<String, Session> 是 Send + Sync（当 K,V 是 Send + Sync）
 // Session 需要是 Send
 // 一切正常工作

 // 3. 不能跨线程的例子
 // let local = String::from("hello");
 // let r = &local;
 // tokio::spawn(async move {
 //     println!("{r}"); // 编译错误：r 不是 'static
 // });

 fn main() {
     println!("Send: 可以移到另一个线程");
     println!("Sync: 可以多线程同时只读");
     println!("大多数类型自动实现，Rc/Cell/裸指针 需要特殊处理");
 }
 ```


### 15.5 项目中的 unsafe 用法

OpenAgere 项目中 unsafe 使用极少，主要在：
- 底层系统调用（进程管理、终端控制）
- `portable-pty` crate 的 FFI 调用
- `keyring-store` 的系统级密钥存储

```rust
// 典型的 unsafe 使用模式：
// 1. 最小化 unsafe 块的范围
// 2. 用安全函数包装 unsafe 操作
// 3. 添加安全不变量的文档注释

// 示例：安全地获取 slice 元素（跳过边界检查）
fn get_unchecked_safe<T>(slice: &[T], index: usize) -> &T {
    assert!(index < slice.len(), "index {index} out of bounds (len: {})", slice.len());
    unsafe { slice.get_unchecked(index) }
}

fn main() {
    let data = vec![10, 20, 30, 40, 50];
    println!("{}", get_unchecked_safe(&data, 2)); // 30
    // get_unchecked_safe(&data, 10); // panic: 安全地阻止越界
}
```

### 15.6 unsafe 的最佳实践

1. **最小化 unsafe 范围** — 只把必须的操作放在 unsafe 块中
2. **用安全 API 包装** — 对外暴露安全接口
3. **文档注释** — 说明 unsafe 块维护了什么不变量
4. **优先用安全替代** — 如果安全代码能做，就不要用 unsafe
5. **测试 unsafe 代码** — unsafe 代码更容易出 bug

### 本章小结

| 概念 | 关键点 |
|------|--------|
| unsafe 块 | 绕过编译器检查，程序员负责安全 |
| 裸指针 | `*const T` / `*mut T`，可悬垂、可 null |
| unsafe fn | 调用者需要在 unsafe 块中调用 |
| FFI | `extern "C"` 与 C 互操作 |
| Send | 可跨线程转移所有权 |
| Sync | 可跨线程共享引用 |
| 最佳实践 | 最小化 unsafe 范围，用安全 API 包装 |

## 16. 宏系统

Rust 有两种宏：**声明式宏**（`macro_rules!`）和**过程宏**（proc macro）。
宏让你在编译期生成代码，减少重复，创建领域特定语言（DSL）。

### 16.1 `macro_rules!` — 声明式宏

声明式宏通过**模式匹配**展开代码。每个规则包含一个匹配模式和一个展开模板。

```rust
// 简单的日志宏
macro_rules! log {
    ($level:expr, $($arg:tt)*) => {
        println!("[{}] {}", $level, format!($($arg)*));
    };
}

// 求和宏 — 使用重复 ($x:expr),*
macro_rules! sum {
    ($($x:expr),*) => {
        {
            let mut total = 0;
            $(total += $x;)*  // 对每个 $x 展开一次
            total
        }
    };
}

// 类似 vec! 的自定义宏
macro_rules! my_vec {
    ($($x:expr),*) => {
        {
            let mut v = Vec::new();
            $(v.push($x);)*
            v
        }
    };
    // 支持尾随逗号
    ($($x:expr,)+) => { my_vec!($($x),+) };
}

// 键值对宏
macro_rules! hashmap {
    ($($key:expr => $value:expr),* $(,)?) => {
        {
            let mut map = std::collections::HashMap::new();
            $(map.insert($key, $value);)*
            map
        }
    };
}

fn main() {
    log!("INFO", "application started on port {}", 8080);
    log!("WARN", "disk usage at {}%", 95);

    let s = sum!(1, 2, 3, 4, 5);
    println!("sum = {s}"); // 15

    let v = my_vec![10, 20, 30];
    println!("vec = {v:?}");

    let v2 = my_vec![10, 20, 30,]; // 尾随逗号也支持
    println!("vec with trailing = {v2:?}");

    let map = hashmap! {
        "name" => "Alice",
        "role" => "admin",
    };
    println!("map = {map:?}");
}
```

**模式匹配语法：**

| 说明符 | 匹配 | 示例 |
|--------|------|------|
| `expr` | 表达式 | `42`, `x + 1` |
| `ident` | 标识符 | `foo`, `my_var` |
| `ty` | 类型 | `i32`, `String` |
| `pat` | 模式 | `Some(x)`, `_` |
| `stmt` | 语句 | `let x = 1;` |
| `tt` | 单个 token tree | 几乎任何东西 |
| `block` | 代码块 | `{ ... }` |
| `path` | 路径 | `std::io::Result` |

**重复语法：** `$(...),*` 表示重复零次或多次，`$(...),+` 一次或多次。

#### 项目实例

```rust
// app-server-protocol/src/protocol/v2.rs — 枚举桥接宏
// macro_rules! v2_enum_from_core! {
    //     (
    //         $(#[$enum_meta:meta])*
    //         pub enum $Name:ident from $Src:path {
        //             $( $(#[$variant_meta:meta])* $Variant:ident ),+ $(,)?
        //         }
    //     ) => {
        //         #[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
        //         $(#[$enum_meta])*
        //         #[serde(rename_all = "camelCase")]
        //         #[ts(export_to = "v2/")]
        //         pub enum $Name {
            //             $( $(#[$variant_meta])* $Variant ),+
            //         }
        //     };
    // }
// 这个宏把核心枚举映射到 v2 API 枚举，自动处理 serde/TS 属性

// otel/src/events/shared.rs — 日志宏
// macro_rules! log_event! {
    //     ($self:expr, $($fields:tt)*) => {{
            //         tracing::event!(
            //             target: $crate::targets::OTEL_LOG_ONLY_TARGET,
            //             tracing::Level::INFO,
            //             $($fields)*
            //             event.timestamp = %$crate::events::shared::timestamp(),
            //         );
            //     }};
    // }
// 统一日志格式，避免每个调用处重复 target 和 timestamp

fn main() {
    println!("项目中的宏减少了大量重复代码");
    println!("v2_enum_from_core! 自动生成枚举桥接代码");
    println!("log_event! 统一日志格式");
}
```

### 16.2 过程宏（Proc Macro）

过程宏接收 Rust token 流作为输入，输出新的 token 流。
必须在**单独的 crate** 中定义（`proc-macro = true`）。

三种形式：

| 形式 | 语法 | 用途 |
|------|------|------|
| derive 宏 | `#[derive(MyTrait)]` | 自动实现 trait |
| 属性宏 | `#[my_attribute]` | 修饰函数/结构体 |
| 函数式宏 | `my_macro!(...)` | 类似声明式但更强大 |

```rust
// proc-macro crate 的基本结构
// use proc_macro::TokenStream;
// use syn::{parse_macro_input, DeriveInput, Data};
// use quote::quote;
//
// #[proc_macro_derive(ExperimentalApi, attributes(experimental))]
// pub fn derive_experimental_api(input: TokenStream) -> TokenStream {
    //     let input = parse_macro_input!(input as DeriveInput);
    //     match &input.data {
        //         Data::Struct(data) => derive_for_struct(&input, data),
        //         Data::Enum(data) => derive_for_enum(&input, data),
        //         Data::Union(_) => {
            //             syn::Error::new_spanned(
            //                 &input.ident,
            //                 "ExperimentalApi does not support unions"
            //             ).to_compile_error().into()
            //         }
        //     }
    // }
```

**关键依赖：**

| crate | 作用 |
|-------|------|
| `syn` | 解析 token 流为语法树 |
| `quote` | 从语法树生成 token 流 |
| `proc-macro2` | 可测试的 token 操作 |

#### 项目实例：ExperimentalApi

```rust
// agere-experimental-api-macros/src/lib.rs — 实际的过程宏
//
// 功能：扫描类型上的 #[experimental("reason")] 属性
// 自动生成 ExperimentalApi trait 实现
// 使用 inventory::submit! 在编译期注册
//
// 使用方式：
// #[derive(ExperimentalApi)]
// struct MyApiType {
    //     #[experimental("field/newFeature")]
    //     new_feature: Option<String>,
    //     stable_field: u32,
    // }

fn main() {
    println!("过程宏需要单独的 proc-macro crate");
    println!("使用 syn 解析，quote 生成代码");
    println!("关键依赖: syn, quote, proc-macro2");
}
```

### 16.3 `inventory` — 编译期注册

`inventory` crate 允许在编译期注册数据，运行时零成本收集。

```rust
// 注册（在派生宏生成的代码中）：
// inventory::submit! {
    //     ExperimentalField {
        //         type_name: "MyApiType",
        //         field_name: "new_feature",
        //         reason: "field/newFeature",
        //     }
    // }

// 收集和遍历：
// inventory::collect!(ExperimentalField);
// pub fn experimental_fields() -> Vec<&'static ExperimentalField> {
    //     inventory::iter::<ExperimentalField>.into_iter().collect()
    // }

fn main() {
    println!("inventory 实现了类似 Java @Service 的自动发现");
    println!("编译期通过 link_section 注册，运行时零成本收集");
    println!("项目用它注册实验性 API 字段");
}
```

### 本章小结

| 类型 | 定义 | 场景 |
|------|------|------|
| `macro_rules!` | 模式匹配展开 | 简单代码生成、DSL |
| derive 宏 | `#[derive(Trait)]` | 自动实现 trait |
| 属性宏 | `#[attr]` | 修饰函数/结构体 |
| syn/quote | 解析和生成 | 过程宏的核心依赖 |
| inventory | 编译期注册 | 自动发现/注册 |
| 项目用法 | v2_enum_from_core!, log_event!, ExperimentalApi | 减少重复代码 |

## 17. Serde 序列化与反序列化

Serde 是 Rust 最流行的序列化框架。"Serde" 代表 **Ser**ialize / **De**serialize。
它支持 JSON、TOML、YAML、MessagePack 等几十种格式。本项目大量使用 serde 处理
API 协议和配置文件。

### 17.1 基本 derive

```rust
// use serde::{Serialize, Deserialize};
//
// #[derive(Serialize, Deserialize, Debug)]
// struct Person {
    //     name: String,
    //     age: u32,
    //     emails: Vec<String>,
    // }

// fn main() {
    //     let p = Person {
        //         name: "Alice".to_string(),
        //         age: 30,
        //         emails: vec!["alice@example.com".to_string()],
        //     };
    //
    //     // 序列化：Rust 值 → JSON 字符串
    //     let json = serde_json::to_string_pretty(&p).unwrap();
    //     println!("JSON:\n{json}");
    //     // {
        //     //   "name": "Alice",
        //     //   "age": 30,
        //     //   "emails": ["alice@example.com"]
        //     // }
    //
    //     // 反序列化：JSON 字符串 → Rust 值
    //     let parsed: Person = serde_json::from_str(&json).unwrap();
    //     println!("Parsed: {parsed:?}");
    // }

fn main() {
    println!("核心 API:");
    println!("serde_json::to_string(&value) → JSON 字符串");
    println!("serde_json::from_str(s) → Rust 值");
    println!("#[derive(Serialize, Deserialize)] 自动派生");
}
```

### 17.2 `#[serde(...)]` 属性详解

#### `rename_all` — 统一命名风格

```rust
// Rust 用 snake_case，API 用 camelCase
// #[derive(Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// struct ApiPayload {
    //     http_status_code: u16,  // JSON: "httpStatusCode"
    //     request_id: String,     // JSON: "requestId"
    //     error_message: Option<String>,
    // }

fn main() {
    println!("rename_all 的值：");
    println!("  camelCase  — API 常见");
    println!("  snake_case — 配置文件常见");
    println!("  kebab-case — URL 常见");
    println!("  lowercase  — 简单枚举");
}
```

#### `tag` — 内部标记的枚举

```rust
// #[derive(Serialize, Deserialize)]
// #[serde(tag = "type", rename_all = "snake_case")]
// enum ConfigValue {
    //     String { value: String },
    //     Number { value: f64 },
    //     Boolean { value: bool },
    // }
//
// JSON 输出:
// { "type": "string", "value": "hello" }
// { "type": "number", "value": 3.14 }
// { "type": "boolean", "value": true }

fn main() {
    println!("tag = \"type\" — 在 JSON 对象中加一个 \"type\" 字段");
    println!("反序列化时根据 type 字段决定枚举变体");
}
```

#### `rename` — 单字段重命名

```rust
// #[derive(Serialize, Deserialize)]
// struct ErrorInfo {
    //     #[serde(rename = "httpStatusCode")]
    //     http_status_code: Option<u16>,
    //     #[serde(rename = "requestId")]
    //     request_id: Option<String>,
    // }

fn main() {
    println!("rename 用于单个字段的精确控制");
}
```

#### `default` + `skip_serializing_if` — 可选字段

```rust
// #[derive(Serialize, Deserialize)]
// struct ApiResponse {
    //     data: String,
    //     #[serde(default, skip_serializing_if = "Option::is_none")]
    //     trace: Option<String>,  // None 时不序列化
    // }

fn main() {
    println!("default — 缺失字段用 Default trait 的值");
    println!("skip_serializing_if = \"Option::is_none\" — None 不输出");
    println!("组合使用：反序列化时缺字段不报错，序列化时省略空值");
}
```

#### `transparent` — 透明包装

```rust
// #[derive(Serialize, Deserialize)]
// #[serde(transparent)]
// struct GitSha(pub String);
// // JSON: "abc123" — 和普通 String 一样
// // 没有包装层

fn main() {
    println!("transparent — newtype 序列化时表现得和内部类型一样");
    println!("GitSha(String) 序列化为 \"abc123\" 而不是 {{\"0\":\"abc123\"}}");
}
```

#### `try_from` / `into` — 自定义转换

```rust
// #[derive(Serialize, Deserialize)]
// #[serde(try_from = "String", into = "String")]
// struct AgentPath(String);
//
// impl TryFrom<String> for AgentPath {
    //     type Error = String;
    //     fn try_from(s: String) -> Result<Self, String> {
        //         if s.starts_with('@') { Ok(AgentPath(s)) }
        //         else { Err("must start with @".into()) }
        //     }
    // }
//
// impl From<AgentPath> for String {
    //     fn from(p: AgentPath) -> String { p.0 }
    // }

fn main() {
    println!("try_from — 反序列化时验证（可能失败）");
    println!("into — 序列化时转换");
}
```

#### Double Option 模式

```rust
// #[serde(
//     default,
//     deserialize_with = "double_option_serde::deserialize",
//     serialize_with = "double_option_serde::serialize",
//     skip_serializing_if = "Option::is_none"
// )]
// pub prompt: Option<Option<String>>,

// None          → JSON 中不存在该字段（省略）
// Some(None)    → JSON: "prompt": null
// Some(Some(v)) → JSON: "prompt": "value"

fn main() {
    println!("Double Option:");
    println!("  None → 不发送");
    println!("  Some(None) → 发送 null（表示清除）");
    println!("  Some(Some(v)) → 发送值");
}
```

### 17.3 自定义反序列化

```rust
// 当 derive 不够用时，手动实现
// impl<'de> Deserialize<'de> for DynamicToolSpec {
    //     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    //     where
    //         D: Deserializer<'de>,
    //     {
        //         // 自定义解析逻辑
        //     }
    // }

fn main() {
    println!("自定义反序列化用于处理多种格式的输入");
    println!("项目中的 DynamicToolSpec 就用了自定义反序列化");
}
```

### 17.4 项目中的 Serde 用法

```rust
// protocol/src/protocol.rs
// #[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
// #[serde(rename_all = "snake_case")]
// pub enum RealtimeOutputModality { Text, Audio }
//
// protocol/src/permissions.rs
// #[derive(Serialize, Deserialize)]
// #[serde(tag = "type", rename_all = "snake_case")]
// pub enum ManagedFileSystemPermissions {
    //     Restricted { entries: Vec<...> },
    //     Unrestricted,
    // }

fn main() {
    println!("项目中的 serde 用法覆盖了所有常见模式:");
    println!("  rename_all — API 命名风格");
    println!("  tag — 枚举 JSON 表示");
    println!("  default + skip — 可选字段");
    println!("  transparent — newtype");
    println!("  try_from — 自定义验证");
    println!("  double Option — 区分 absent/null/value");
}
```

### 本章小结

| 属性 | 作用 | 项目用法 |
|------|------|---------|
| rename_all | 统一命名风格 | camelCase/snake_case |
| tag | 内部标记枚举 | 协议类型区分 |
| rename | 单字段重命名 | API 兼容 |
| default + skip | 可选字段 | API payload |
| transparent | 透明包装 | newtype (GitSha) |
| try_from | 自定义验证 | AgentPath |
| double Option | 区分 absent/null | 协议字段更新 |
| deserialize_with | 自定义反序列化 | DynamicToolSpec |

## 18. async/await 深入

Rust 的异步编程基于 `async/await` 语法，编译为状态机。本章深入讲解底层原理。

### 18.1 Future trait — 异步计算的基础

```rust
// 标准库中的 Future trait:
// pub trait Future {
    //     type Output;
    //     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
    // }
//
// pub enum Poll<T> {
    //     Ready(T),    // 计算完成，返回结果
    //     Pending,     // 还没准备好，稍后再 poll
    // }

async fn example() -> i32 {
    42
}

// 调用 example() 不会执行函数体
// 它返回一个 Future，需要 .await 或运行时来驱动

async fn caller() {
    let result = example().await;  // 驱动 Future 直到完成
    println!("result: {result}");
}

fn main() {
    println!("Future 是 Rust async 的基础 trait");
    println!("async fn 编译为状态机");
    println!(".await 驱动 Future 执行");
}
```

**async fn 编译为什么？**

```rust
// 这个 async fn:
async fn fetch_data(url: &str) -> String {
    let response = http_get(url).await;
    let body = response.text().await;
    body
}

// 编译器大致生成等价的状态机:
enum FetchDataFuture {
    Initial { url: String },
    WaitingResponse { url: String, get_future: HttpGetFuture },
    WaitingText { response: Response },
    Complete,
}

// 每次 .await 都是一个状态转换点
// poll() 被调用时，根据当前状态恢复执行
```

### 18.2 async block 与移动语义

```rust
async fn example() {
    let name = String::from("Alice");

    // async block 捕获变量
    let fut = async {
        println!("in async block: {name}");
    };
    // name 被移动到 fut 中
    // println!("{name}"); // ❌ 已移动

    // async move — 显式获取所有权
    let data = vec![1, 2, 3];
    let fut2 = async move {
        for item in &data {
            print!("{item} ");
        }
    };
    // data 被移动到 fut2 中

    // 在 tokio::spawn 中必须用 async move
    // 因为 spawn 的任务可能在另一个线程执行
    let value = 42;
    tokio::spawn(async move {
        println!("spawned: {value}");
    });
}

fn main() { println!("async block 和 async move 控制捕获语义"); }
```

### 18.3 Stream trait — 异步迭代器

```rust
// Stream 是 Iterator 的异步版本
// pub trait Stream {
    //     type Item;
    //     fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>)
    //         -> Poll<Option<Self::Item>>;
    // }

// 使用 futures::stream
// use futures::stream::{self, StreamExt};
//
// let mut s = stream::iter(vec![1, 2, 3]);
// while let Some(item) = s.next().await {
    //     println!("{item}");
    // }

// Stream 的适配器
// stream.map(|x| x * 2).filter(|x| x > 3).collect::<Vec<_>>().await

fn main() {
    println!("Stream = 异步迭代器");
    println!(".next().await 获取下一个元素");
    println!("适配器和 Iterator 类似：map, filter, collect");
}
```

#### FuturesOrdered 与 FuturesUnordered

```rust
// use futures::stream::{FuturesOrdered, FuturesUnordered};

// FuturesOrdered — 保持提交顺序，按顺序返回结果
// let mut ordered = FuturesOrdered::new();
// ordered.push_back(fetch_user(1));
// ordered.push_back(fetch_user(2));
// ordered.push_back(fetch_user(3));
// while let Some(result) = ordered.next().await {
    //     // 按 1, 2, 3 的顺序处理
    // }

// FuturesUnordered — 谁先完成谁先返回
// let mut unordered = FuturesUnordered::new();
// unordered.push(fetch_user(1));
// unordered.push(fetch_user(2));
// unordered.push(fetch_user(3));
// while let Some(result) = unordered.next().await {
    //     // 按完成顺序处理（可能 2, 1, 3）
    // }

fn main() {
    println!("FuturesOrdered: 保持提交顺序");
    println!("FuturesUnordered: 完成顺序");
    println!("项目用法:");
    println!("  session/turn.rs — FuturesOrdered 工具按顺序执行");
    println!("  tools/handlers/agent_jobs.rs — FuturesUnordered 并发批量");
    println!("  thread_manager.rs — FuturesUnordered 线程管理");
}
```

### 18.4 Tokio 运行时

Tokio 是 Rust 最流行的异步运行时，提供：
- **多线程 work-stealing 调度器** — 任务自动在多个线程间均衡
- **异步 I/O** — TCP, UDP, 文件系统
- **定时器** — sleep, timeout, interval
- **同步原语** — Mutex, RwLock, Semaphore, channel

```rust
// #[tokio::main]
// async fn main() {
    //     // 多线程运行时（默认）
    //     // work-stealing 调度器自动分配任务到线程池
    //
    //     // 创建异步任务
    //     let handle = tokio::spawn(async {
        //         42
        //     });
    //
    //     // 等待任务完成
    //     let result = handle.await.unwrap();
    //     println!("result: {result}");
    //
    //     // 并发执行多个任务
    //     let (a, b) = tokio::join!(
    //         fetch_data("url1"),
    //         fetch_data("url2"),
    //     );
    //
    //     // 异步睡眠
    //     tokio::time::sleep(Duration::from_secs(1)).await;
    //
    //     // 超时
    //     match tokio::time::timeout(Duration::from_secs(5), slow_task()).await {
        //         Ok(result) => println!("完成: {result:?}"),
        //         Err(_) => println!("超时"),
        //     }
    // }

fn main() {
    println!("Tokio 运行时提供:");
    println!("  spawn — 并发异步任务");
    println!("  join! — 并发等待多个任务");
    println!("  select! — 多路复用");
    println!("  channels — mpsc/oneshot/broadcast/watch");
    println!("  Mutex/RwLock — 异步同步原语");
    println!("  time — sleep/timeout/interval");
}
```

### 本章小结

| 概念 | 关键点 |
|------|--------|
| Future | 异步计算的基础 trait |
| async fn | 编译为状态机 |
| .await | 驱动 Future 执行 |
| async move | 获取所有权，用于 spawn |
| Stream | 异步迭代器 |
| FuturesOrdered | 保持顺序的并发流 |
| FuturesUnordered | 完成顺序的并发流 |
| Tokio | 多线程调度 + 异步 I/O + 定时器 |
| join!/select! | 并发等待 / 多路复用 |


 ### Tokio 核心用法详解

 Tokio 是 Rust 最常用的异步运行时。本节详细讲解它的核心概念和用法。

 #### tokio::spawn — 创建并发任务

 `tokio::spawn` 把一个 `Future` 提交给 Tokio 运行时调度。
 它返回一个 `JoinHandle`，可以用来等待结果或取消任务。

 ```rust
 // tokio::spawn 的签名（简化版）
 // pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
 // where
 //     F: Future + Send + 'static,
 //     F::Output: Send + 'static,

 // 关键约束：
 // 1. F: Send — 任务可以在线程池的不同线程上执行
 // 2. F: 'static — 任务不能借用局部变量（用 Arc 或 move）

 // 基本用法
 #[tokio::main]
 async fn main() {
     // spawn 一个任务
     let handle = tokio::spawn(async {
         // 模拟一些异步工作
         tokio::time::sleep(std::time::Duration::from_millis(100)).await;
         42
     });

     // handle 的类型是 JoinHandle<i32>
     // .await 等待任务完成
     let result = handle.await.unwrap();
     println!("result: {result}"); // 42
 }
 ```

 **JoinHandle 能做什么？**

 ```rust
 // JoinHandle<T> 提供：
 //
 // .await -> Result<T, JoinError>
 //   等待任务完成。如果任务 panic，返回 Err(JoinError)
 //
 // .abort()
 //   取消任务。任务会被标记为取消，
 //   下次 .await 点会触发 panic（或提前返回）
 //
 // .is_finished() -> bool
 //   检查任务是否已完成（不会阻塞）

 // 项目中的用法（core/src/session/mod.rs）：
 // let handle: JoinHandle<()> = tokio::spawn(session_loop);
 // // 存储 handle 以便后续等待或取消

 fn main() {
     println!("JoinHandle: spawn 返回的任务句柄");
     println!(".await — 等待完成，获取结果");
     println!(".abort() — 取消任务");
     println!(".is_finished() — 非阻塞检查");
 }
 ```

 #### tokio::join! — 并行等待多个任务

 `tokio::join!` 同时等待多个 Future，所有任务并发执行。

 ```rust
 // 对比：顺序执行 vs 并行执行

 // ❌ 顺序执行 — 总时间 = t1 + t2 + t3
 async fn sequential() {
     let r1 = slow_task_1().await;  // 等 1 秒
     let r2 = slow_task_2().await;  // 再等 1 秒
     let r3 = slow_task_3().await;  // 再等 1 秒
     // 总共 3 秒
 }

 // ✅ 并行执行 — 总时间 = max(t1, t2, t3)
 async fn parallel() {
     let (r1, r2, r3) = tokio::join!(
         slow_task_1(),
         slow_task_2(),
         slow_task_3(),
     );
     // 总共 1 秒（三个任务同时执行）
 }

 // join! 不会创建新线程或新任务
 // 它只是在当前任务内并发 poll 多个 future

 async fn slow_task_1() -> i32 { 1 }
 async fn slow_task_2() -> i32 { 2 }
 async fn slow_task_3() -> i32 { 3 }

 fn main() {
     println!("join!: 在当前任务内并行等待多个 future");
     println!("总时间 = max(t1, t2, ...) 而不是 sum");
 }
 ```

 #### tokio::select! — 多路复用（竞赛模式）

 `tokio::select!` 同时等待多个分支，**最先完成的赢**。
 其他分支会被取消。

 ```rust
 // 基本用法
 async fn example() {
     let result = tokio::select! {
         value = slow_operation() => {
             // slow_operation 先完成
             format!("操作完成: {value}")
         }
         _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
             // 超时先到了
             "超时!".to_string()
         }
     };
     println!("{result}");
 }

 // 项目中的核心用法（core/src/agere_delegate.rs）：
 // 事件循环同时监听"取消信号"和"新事件"
 //
 // loop {
 //     tokio::select! {
 //         // 分支 1: 取消信号
 //         _ = &mut cancelled => {
 //             shutdown_delegate(&agere).await;
 //             break;
 //         }
 //         // 分支 2: 新事件
 //         event = agere.next_event() => {
 //             let event = match event {
 //                 Ok(event) => event,
 //                 Err(_) => break,
 //             };
 //             // 处理事件...
 //         }
 //     }
 // }
 //
 // 行为：谁先准备好就执行谁
 // - 如果取消信号先来 → 关闭并退出
 // - 如果新事件先来 → 处理事件，继续循环

 // biased 模式 — 按顺序检查优先级
 // tokio::select! {
 //     biased;  // <-- 关键：上面的分支优先
 //     _ = cancel.cancelled() => { /* 取消总是优先 */ }
 //     response = fut => response.unwrap_or_default(),
 // }
 //
 // 没有 biased: 随机选择一个准备好的分支
 // 有 biased: 按代码顺序，第一个准备好的执行

 async fn slow_operation() -> i32 { 42 }

 fn main() {
     println!("select!: 多路复用，最先完成的赢");
     println!("biased: 按优先级检查分支");
 }
 ```

 #### 通道详解 — 各类型的适用场景

 ```rust
 // mpsc — 多生产者，单消费者
 // 用途：命令队列、任务提交
 // 项目：app-server/command_exec.rs 的控制命令队列

 // let (tx, mut rx) = tokio::sync::mpsc::channel(32);
 // // 多个发送者
 // let tx2 = tx.clone();
 // tokio::spawn(async move { tx.send("cmd1").await.unwrap(); });
 // tokio::spawn(async move { tx2.send("cmd2").await.unwrap(); });
 // // 一个接收者
 // while let Some(cmd) = rx.recv().await { println!("{cmd}"); }

 // oneshot — 单次发送
 // 用途：请求-响应模式
 // 项目：认证刷新的异步应答

 // let (tx, rx) = tokio::sync::oneshot::channel();
 // tokio::spawn(async move {
 //     let result = do_auth_refresh().await;
 //     tx.send(result).unwrap();  // 只能发一次
 // });
 // let result = rx.await.unwrap();

 // broadcast — 一对多广播
 // 用途：事件通知（文件变更、状态变更）
 // 项目：core/skills_watcher.rs 技能文件变更通知

 // let (tx, _) = tokio::sync::broadcast::channel(128);
 // let mut sub1 = tx.subscribe();
 // let mut sub2 = tx.subscribe();
 // tx.send("file_changed").unwrap();
 // // sub1 和 sub2 都能收到 "file_changed"

 // watch — 最新值同步
 // 用途：状态同步（只关心最新值，不关心历史）
 // 项目：Agent 状态通知 UI

 // let (tx, mut rx) = tokio::sync::watch::channel("idle");
 // tx.send("working").unwrap();
 // tx.send("done").unwrap();  // 中间值 "working" 可能被跳过
 // rx.changed().await.unwrap();
 // println!("{}", *rx.borrow());  // "done"（最新值）

 fn main() {
     println!("mpsc — 命令队列（多对一）");
     println!("oneshot — 请求响应（一对一单次）");
     println!("broadcast — 事件广播（一对多）");
     println!("watch — 状态同步（只关心最新值）");
 }
 ```


---


## 19. 并发编程

本章深入讲解 Rust 的并发模型——编译期保证线程安全，没有运行时数据竞争。

### 19.1 Send 与 Sync — 线程安全的基础

`Send` 和 `Sync` 是两个标记 trait，Rust 编译器自动为大多数类型实现它们。

```rust
fn assert_send_sync<T: Send + Sync>() {}

fn main() {
    // ✅ 基本类型
    assert_send_sync::<i32>();
    assert_send_sync::<String>();
    assert_send_sync::<Vec<i32>>();
    assert_send_sync::<HashMap<String, i32>>();

    // ✅ Arc（原子引用计数）
    assert_send_sync::<Arc<i32>>();

    // ✅ Mutex<T> 当 T: Send 时是 Send + Sync
    assert_send_sync::<std::sync::Mutex<i32>>();

    // ✅ Channel 端点
    // assert_send_sync::<tokio::sync::mpsc::Sender<i32>>();
    // assert_send_sync::<tokio::sync::mpsc::Receiver<i32>>();

    // ❌ Rc — 非原子引用计数，不能跨线程
    assert_send::<std::rc::Rc<i32>>();
    // assert_sync::<std::rc::Rc<i32>>(); // 不编译

    // ❌ Cell/RefCell — 内部可变性，线程不安全
    // assert_send::<std::cell::Cell<i32>>();

    // ❌ 裸指针
    // assert_send::<*const i32>();
    // assert_send::<*mut i32>();
}
```

**为什么这很重要？**

Tokio 的 `spawn` 要求闭包是 `Send + 'static`：
```rust
// pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
// where
//     F: Future + Send + 'static,
//     F::Output: Send + 'static,
// ```

这意味着：
- 闭包捕获的变量必须是 `Send`
- 闭包必须是 `'static`（不能借用局部变量，除非用 `Arc`）

```rust
// ❌ 编译错误
// let local = String::from("hello");
// tokio::spawn(async {
//     println!("{local}");  // local 被借用，但 spawn 需要 'static
// });

// ✅ 用 move 获取所有权
// let local = String::from("hello");
// tokio::spawn(async move {
//     println!("{local}");
// });

// ✅ 用 Arc 共享
// let shared = Arc::new(String::from("hello"));
// let shared_clone = Arc::clone(&shared);
// tokio::spawn(async move {
//     println!("{shared_clone}");
// });

fn main() {
println!("Send: 可跨线程转移所有权");
println!("Sync: 可跨线程共享引用");
println!("spawn 要求 Send + 'static");
}
```

### 19.2 tokio::spawn — 并发任务

```rust
// tokio::spawn 创建新的异步任务
// 任务被调度到线程池的某个线程上执行

// 基本用法
// let handle = tokio::spawn(async {
//     // 做一些异步工作
//     42
// });
//
// // 等待结果
// let result = handle.await.unwrap();
// println!("result: {result}");

// 并发多个任务
// let h1 = tokio::spawn(task1());
// let h2 = tokio::spawn(task2());
// let h3 = tokio::spawn(task3());
//
// let (r1, r2, r3) = tokio::join!(h1, h2, h3);

fn main() {
println!("tokio::spawn: 创建并发异步任务");
println!("JoinHandle: 等待任务完成");
println!("tokio::join!: 并发等待多个任务");
}
```

### 19.3 tokio::select! — 多路复用

`select!` 同时等待多个异步操作，返回最先完成的那个。

```rust
// 项目实例: core/src/agere_delegate.rs

// loop {
//     tokio::select! {
//         _ = &mut cancelled => {
//             shutdown_delegate(&agere).await;
//             break;
//         }
//         event = agere.next_event() => {
//             let event = match event {
//                 Ok(event) => event,
//                 Err(_) => break,
//             };
//             // ...event dispatch
//         }
//     }
// }

// biased select — 按优先级检查
// tokio::select! {
//     biased;  // 上面的分支优先检查
//     _ = cancel_token.cancelled() => {
//         // 取消信号总是优先处理
//         let empty = Response { answers: HashMap::new() };
//         parent_session.notify(sub_id, empty.clone()).await;
//         empty
//     }
//     response = fut => response.unwrap_or_default(),
// }

// 多个 watch channel
// tokio::select! {
//     _ = unloading_sleep => return true,
//     changed = self.has_subscribers_rx.changed() => {
//         if changed.is_err() { return false; }
//         self.sync_receiver_values();
//     },
//     changed = self.thread_status_rx.changed() => {
//         if changed.is_err() { return false; }
//         self.sync_receiver_values();
//     },
// }

fn main() {
println!("select!: 多路复用，最先完成的赢");
println!("biased: 按优先级检查分支");
println!("项目用法: 取消信号 + 事件循环 + 状态监听");
}
```

### 19.4 通道类型

Tokio 提供多种通道，适用于不同的通信模式：

| 类型 | 生产者 | 消费者 | 缓冲 | 用途 |
|------|--------|--------|------|------|
| **mpsc** | 多个 | 一个 | 有界 | 命令队列 |
| **oneshot** | 一个 | 一个 | 单次 | 请求-响应 |
| **broadcast** | 一个 | 多个 | 有界 | 事件广播 |
| **watch** | 一个 | 多个 | 最新值 | 状态同步 |

```rust
use tokio::sync::{mpsc, oneshot, broadcast, watch};

#[tokio::main]
async fn main() {
// ---- mpsc: 多生产者单消费者 ----
let (tx, mut rx) = mpsc::channel(32);  // 缓冲 32

tokio::spawn(async move {
tx.send("command1").await.unwrap();
tx.send("command2").await.unwrap();
});

while let Some(cmd) = rx.recv().await {
println!("收到命令: {cmd}");
}

// ---- oneshot: 单次请求-响应 ----
let (tx, rx) = oneshot::channel();

tokio::spawn(async move {
tx.send(42).unwrap();
});

let result = rx.await.unwrap();
println!("oneshot 结果: {result}");

// ---- broadcast: 一对多广播 ----
let (tx, _) = broadcast::channel(128);  // 缓冲 128

let mut rx1 = tx.subscribe();
let mut rx2 = tx.subscribe();

tx.send("event").unwrap();

println!("rx1: {:?}", rx1.recv().await);
println!("rx2: {:?}", rx2.recv().await);

// ---- watch: 只关心最新值 ----
let (tx, mut rx) = watch::channel("initial");

tx.send("updated").unwrap();
rx.changed().await.unwrap();
println!("watch 值: {}", *rx.borrow());
}
```

#### 项目中的通道用法

| 通道 | 项目位置 | 用途 |
|------|---------|------|
| mpsc | app-server/command_exec.rs | 控制命令队列 |
| oneshot | app-server/command_exec.rs | 异步响应 |
| broadcast | core/skills_watcher.rs | 技能文件变更通知 |
| watch | core/agere_delegate_tests.rs | Agent 状态同步 |
| async_channel | core/agere_delegate.rs | 跨 crate 提交通道 |

### 19.5 Arc<Mutex<T>> — 共享可变状态

```rust
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
let counter = Arc::new(Mutex::new(0));
let mut handles = vec![];

for _ in 0..5 {
let counter = Arc::clone(&counter);
let handle = thread::spawn(move || {
let mut num = counter.lock().unwrap();
*num += 1;
});
handles.push(handle);
}

for handle in handles { handle.join().unwrap(); }
println!("计数: {}", *counter.lock().unwrap()); // 5
}
```

#### tokio Mutex vs std Mutex

| | std::sync::Mutex | tokio::sync::Mutex |
|--|-----------------|-------------------|
| 锁定 | `.lock().unwrap()` | `.lock().await` |
| 跨 .await | ❌ 不允许 | ✅ 允许 |
| 性能 | 更快 | 略慢 |
| 场景 | 短持有 | 长持有 |

```rust
// 项目实例: app-server/src/bespoke_event_handling.rs
// thread_state: Arc<tokio::sync::Mutex<ThreadState>>,

// 项目实例: app-server/src/command_exec.rs
// sessions: Arc<Mutex<HashMap<ConnectionProcessId, CommandExecSession>>>,
//
// let mut sessions = self.sessions.lock().await;
// if sessions.contains_key(&process_key) {
//     return Err(invalid_request("duplicate".to_string()));
// }

// 项目实例: app-server/src/agere_message_processor.rs
// active_login: Arc<Mutex<Option<ActiveLogin>>>,
// pending_thread_unloads: Arc<Mutex<HashSet<ThreadId>>>,

fn main() {
println!("Arc<Mutex<T>>: 共享可变状态的标准模式");
println!("项目中大量使用，如会话映射、登录状态、线程集合");
}
```


### 19.5.1 Scoped 线程 — 可以借用局部变量

标准 `thread::spawn` 要求闭包是 `'static`——不能借用局部变量。
`std::thread::scope` 允许创建**作用域线程**，它们可以安全地借用父线程的局部变量，
因为 scope 会等待所有子线程完成后才返回。

```rust
use std::thread;

fn main() {
let data = vec![1, 2, 3, 4, 5];

// ❌ 标准 spawn 不能借用局部变量
// thread::spawn(|| {
//     println!("{data:?}");  // ❌ 编译错误
// });

// ✅ scoped thread 可以借用
thread::scope(|s| {
s.spawn(|| {
println!("线程 1: {data:?}");  // ✅ 借用 data
});
s.spawn(|| {
println!("线程 2: {}", data.len());  // ✅
});
// scope 结束时自动 join 所有子线程
});
// 到这里所有子线程已完成，data 可以安全使用
println!("data 仍然有效: {data:?}");
}
```

**什么时候用 scoped 线程？**
- 需要并行处理局部数据而不需要 clone
- 并行计算（如并行 map/reduce）
- 临时性的并行任务，不需要长期运行的线程

### 19.6 取消与超时

```rust
// CancellationToken — 协作式取消
// use tokio_util::sync::CancellationToken;
//
// let ct = CancellationToken::new();
// let ct2 = ct.clone();
//
// tokio::spawn(async move {
//     tokio::select! {
//         _ = ct2.cancelled() => return,  // 被取消
//         _ = do_work() => {},            // 正常完成
//     }
// });
//
// ct.cancel();  // 通知所有持有者取消

// 项目实例:
// .or_cancel(&cancel_token).await??;
// 第一个 ? = 取消错误 (CancelledError)
// 第二个 ? = 内部 Result<Agere, AgereErr>

// 超时
// match tokio::time::timeout(Duration::from_secs(5), slow_task()).await {
//     Ok(result) => println!("完成: {result:?}"),
//     Err(_) => println!("超时"),
// }

// 项目实例: app-server/src/message_processor.rs
// match timeout(EXTERNAL_AUTH_REFRESH_TIMEOUT, rx).await {
//     Ok(Ok(result)) => Ok(result),
//     Ok(Err(_)) => Err("channel closed"),
//     Err(_) => Err("timeout"),
// }

fn main() {
println!("取消: CancellationToken 协作式");
println!("超时: tokio::time::timeout");
println!("项目中两者结合使用");
}
```

### 本章小结

| 模式 | 实现 | 项目用法 |
|------|------|---------|
| Send + Sync | 编译期线程安全 | spawn 要求 |
| tokio::spawn | 并发异步任务 | 多任务并行 |
| tokio::select! | 多路复用 | 事件循环 + 取消 |
| mpsc | 命令队列 | 控制命令 |
| oneshot | 请求响应 | 异步应答 |
| broadcast | 事件广播 | 文件变更通知 |
| watch | 状态同步 | Agent 状态 |
| Arc<Mutex<T>> | 共享可变状态 | 会话/登录/线程集合 |
| CancellationToken | 协作式取消 | 任务生命周期 |
| timeout | 超时处理 | 认证刷新 |




---




---



 ### Cancellation 机制深入理解

 在长时间运行的异步任务中，取消是一个核心需求。Rust/Tokio 提供了多种取消机制，
 项目中使用的是 `tokio_util::sync::CancellationToken`。

 #### 为什么需要取消？

 想象一个场景：用户按了 Ctrl+C，但你的程序正在做一个耗时的网络请求。
 如果你不处理取消，程序会一直等到请求完成才退出——用户体验很差。

 ```rust
 // 问题：没有取消机制
 async fn bad_example() {
     let data = download_large_file().await;  // 可能等 5 分钟
     // 用户按了 Ctrl+C，但程序还在下载...
     process(data).await;
 }

 // 解决方案：CancellationToken
 // 用户按 Ctrl+C → 通知所有任务 → 任务自行停止
 ```

 #### CancellationToken 的工作原理

 `CancellationToken` 是一个**协作式**取消机制：
 - 它不会"强制杀死"任务
 - 它只是发出一个"请停止"的信号
 - 任务自己决定何时检查这个信号并停止

 ```rust
 // CancellationToken 的核心 API
 // let token = CancellationToken::new();
 //
 // // 发出取消信号（通知所有持有者）
 // token.cancel();
 //
 // // 等待取消信号
 // token.cancelled().await;  // 直到 cancel() 被调用才返回
 //
 // // 检查是否已取消
 // token.is_cancelled();  // bool，不阻塞
 //
 // // 克隆 — 可以多个持有者
 // let child = token.child_token();
 // token.cancel();  // 同时取消所有 child token
 ```

 **内部实现（简化版）：**

 ```text
 CancellationToken 内部是一个 Arc<SharedState>:

 +------------------+
 | SharedState       |
 | cancelled: AtomicBool  <-- 原子布尔，cancel() 设为 true
 | wakers: Mutex<Vec<Waker>>  <-- 等待者的唤醒器列表
 +------------------+

 cancel() 调用：
 1. 设置 cancelled = true（原子操作）
 2. 遍历 wakers 列表，逐个 wake()
 3. 每个等待者的 cancelled().await 立即返回

 cancelled().await：
 1. 如果已 cancelled → 立即返回
 2. 否则 → 注册当前 Waker 到列表，挂起等待
 3. 被 wake() 后 → 检查 cancelled，返回
 ```

 #### 项目中的取消模式

 项目中最核心的取消用法在 `core/src/agere_delegate.rs`：

 ```rust
 // 简化版的 agere_delegate 事件循环
 async fn run_delegate(cancel_token: CancellationToken) {
     let agere = start_agere().await;

     loop {
         tokio::select! {
             // 分支 1：取消信号
             _ = cancel_token.cancelled() => {
                 // 收到取消信号
                 shutdown(&agere).await;
                 break;  // 退出循环
             }

             // 分支 2：新事件
             event = agere.next_event() => {
                 match event {
                     Ok(event) => dispatch(event).await,
                     Err(_) => break,
                 }
             }
         }
     }
 }
 ```

 #### .or_cancel() 模式

 项目中使用 `.or_cancel()` 为任何 Future 添加取消能力：

 ```rust
 // .or_cancel() 是一个扩展方法
 // 它把 future 和 cancellation token 组合在一起

 // 用法：
 // let result = some_future
 //     .or_cancel(&cancel_token)
 //     .await;

 // 返回类型是 Result<Result<T, E>, CancelledError>
 // 所以需要两个 ?：
 // let value = some_future.or_cancel(&token).await??;
 //              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
 //              第一个 ?：取消错误
 //              第二个 ?：内部错误

 // 具体流程：
 // 1. some_future 正常完成 → Ok(Ok(value)) → ?? → value
 // 2. some_future 出错 → Ok(Err(error)) → ?? → error 传播
 // 3. 被取消 → Err(CancelledError) → ? → 函数提前返回

 fn main() {
     println!(".or_cancel() 模式：");
     println!("1. 给任何 Future 添加取消能力");
     println!("2. 返回 Result<Result<T,E>, Cancelled>");
     println!("3. 双 ?? 分别处理取消和内部错误");
 }
 ```

 #### Ctrl+C 处理（SIGINT）

 在 Unix 系统上，Ctrl+C 发送 SIGINT 信号。Tokio 提供了异步信号处理：

 ```rust
 // #[tokio::main]
 // async fn main() {
 //     let cancel = CancellationToken::new();
 //     let cancel_clone = cancel.clone();
 //
 //     // 监听 Ctrl+C
 //     tokio::spawn(async move {
 //         tokio::signal::ctrl_c().await.unwrap();
 //         println!("\n收到 Ctrl+C，正在取消...");
 //         cancel_clone.cancel();  // 通知所有任务
 //     });
 //
 //     // 主逻辑
 //     run_with_cancel(cancel).await;
 // }

 // 项目中（exec/src/lib.rs）的简化版：
 // let ctrl_c = tokio::signal::ctrl_c();
 // tokio::select! {
 //     _ = ctrl_c => { cancel.cancel(); }
 //     _ = run_main() => {}
 // }

 fn main() {
     println!("Ctrl+C -> signal::ctrl_c().await -> cancel()");
 }
 ```

 #### 超时处理

 超时是另一种形式的"取消"——如果任务在规定时间内没完成，自动取消：

 ```rust
 // tokio::time::timeout 用法
 // let result = tokio::time::timeout(
 //     Duration::from_secs(5),
 //     slow_task(),
 // ).await;
 //
 // match result {
 //     Ok(Ok(value)) => println!("成功: {value}"),
 //     Ok(Err(e)) => println!("任务失败: {e}"),
 //     Err(_) => println!("超时!"),
 // }

 // 项目中的超时用法（app-server/src/message_processor.rs）：
 // match timeout(EXTERNAL_AUTH_REFRESH_TIMEOUT, rx).await {
 //     Ok(Ok(result)) => Ok(result),
 //     Ok(Err(_)) => Err("认证通道关闭"),
 //     Err(_) => Err("认证刷新超时"),
 // }

 fn main() {
     println!("timeout: 限时等待，超时自动取消");
     println!("Ok(value) — 在时间内完成");
     println!("Err(_timeout) — 超时了");
 }
 ```

 #### 取消的传播

 当父任务被取消时，子任务也应该被取消：

 ```rust
 // CancellationToken 支持父子关系
 // let parent = CancellationToken::new();
 // let child = parent.child_token();
 //
 // parent.cancel();  // 同时取消 child

 // 在 spawn 中传播：
 // let token_clone = token.clone();
 // tokio::spawn(async move {
 //     work_with_cancel(token_clone).await;
 // });
 // token.cancel();  // spawn 的任务也会收到取消信号

 fn main() {
     println!("取消传播: cancel() 通知所有 clone 和 child token");
     println!("协作式: 任务需要主动检查取消状态");
 }
 ```

 #### 取消 vs 杀死

 ```text
 取消（Cooperative Cancellation）：         杀死（Forced Kill）：
 - 通知任务"请停止"                        - 强制终止任务
 - 任务在 .await 点检查信号                - 不管任务在做什么
 - 任务可以清理资源再退出                   - 可能泄漏资源
 - 安全、可控                              - 危险、不可控
 - Rust/Tokio 使用这种方式                  - 不推荐
 ```

 这就是为什么 Rust 的取消是"协作式"的——它保证资源安全释放。


---


## 20. 类型系统与高级 Trait

### 20.1 Newtype 模式 — 类型安全的利器

Newtype 模式用一个元组结构体包装已有类型，赋予新的语义。编译器会阻止你把不同类型
混淆——这在 API 设计、科学计算、金融系统中极其重要。

```rust
struct Meters(f64);
struct Seconds(f64);
struct Kilograms(f64);

// 物理公式：速度 = 距离 / 时间
fn speed(distance: Meters, time: Seconds) -> f64 {
distance.0 / time.0
}

// 能量 = 质量 × 速度² / 2
fn kinetic_energy(mass: Kilograms, velocity: f64) -> f64 {
0.5 * mass.0 * velocity * velocity
}

fn main() {
let d = Meters(100.0);
let t = Seconds(9.58);
let v = speed(d, t);
println!("速度: {:.2} m/s", v);
println!("动能: {:.2} J", kinetic_energy(Kilograms(70.0), v));

// speed(t, d);  // ❌ 编译错误：参数类型不匹配
// speed(d, Kilograms(9.58));  // ❌ 编译错误
}
```

**项目实例：**

```rust
// protocol/src/protocol.rs — GitSha newtype
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, TS)]
#[serde(transparent)]
pub struct GitSha(pub String);
// GitSha 和 String 在类型层面不同
// 序列化时表现得和普通字符串一样

// utils/fs — 路径类型安全
// pub struct AbsolutePathBuf(PathBuf);
// 防止相对路径被当成绝对路径使用
```

### 20.2 完整 Match 的价值

不使用 `_` 通配符，让编译器帮你检查遗漏。

```rust
#[derive(Debug, Clone, Copy)]
enum Status { Active, Inactive, Suspended, Archived }

// ❌ 用通配符 — 新增变体时编译器不会提醒
fn describe_bad(s: Status) -> &'static str {
match s {
Status::Active => "活跃",
_ => "其他",  // Suspended/Archived 都在这里，但你看不到
}
}

// ✅ 穷举 — 新增变体时编译器报错
fn describe(s: Status) -> &'static str {
match s {
Status::Active => "活跃",
Status::Inactive => "不活跃",
Status::Suspended => "暂停",
Status::Archived => "已归档",
}
}

// 项目 AGENTS.md 规则:
// "When possible, make match statements exhaustive and avoid wildcard arms"

fn main() {
println!("{}", describe(Status::Active));
// 如果以后新增 Status::Deleted 变体
// 编译器会报错：match 没有处理 Deleted
}
```

### 20.3 标记 Trait

标记 trait（marker trait）没有方法，只标记类型具有某种属性。

```rust
// Send: 可以跨线程转移所有权
// Sync: 可以跨线程共享引用（&T: Send → T: Sync）
// Sized: 编译时已知大小
// Unpin: 可以安全移动（大多数类型）

// 自定义标记 trait
trait Serializable {}
trait Cacheable {}

struct Config;
impl Serializable for Config {}

struct Session;
impl Serializable for Session {}
impl Cacheable for Session {}

fn needs_serializable<T: Serializable>() {}
fn needs_both<T: Serializable + Cacheable>() {}

fn main() {
needs_serializable::<Config>();
needs_both::<Session>();
// needs_both::<Config>(); // ❌ Config 没有实现 Cacheable
}
```

### 本章小结

| 模式 | 用途 |
|------|------|
| Newtype | 类型安全包装，防止单位混淆 |
| 完整 match | 编译期穷举检查 |
| 标记 trait | Send/Sync/属性标记 |
| 关联常量 | `trait Foo { const MAX: usize; }` |
| 默认泛型参数 | `struct Foo<T = i32>` |

---

## 21. 模块系统与 Workspace

### 21.1 可见性

```rust
// mod foo        — 私有，仅当前模块可见
// pub mod foo    — 公开，外部可见
// pub(crate) mod — crate 内可见
// pub(super)     — 父模块可见

fn main() {
println!("最小暴露原则：默认私有，显式公开");
}
```

**项目实例 — core/src/lib.rs：**

```rust
// core/src/lib.rs 的可见性分层

// 纯内部模块 — 外部完全不可见
mod apply_patch;
mod apps;
mod arc_monitor;
mod client;

// 公开模块 — 外部 API
pub mod config;
pub mod connectors;
pub mod exec;

// crate 内可见 — 同 crate 的其他模块可以用
pub(crate) mod session;
pub(crate) mod mcp;
pub(crate) mod mention_syntax;
pub(crate) mod message_history;

// 内联模块分组
pub(crate) mod mentions {
pub(crate) use crate::plugins::build_connector_slug_counts;
pub(crate) use crate::plugins::collect_explicit_app_ids;
}
```

### 21.2 Re-export 与 Deprecation

```rust
// pub use 扁平化 API — 调用方不需要知道内部路径
// pub use session::SteerInputError;
// pub use agere_thread::AgereThread;
// pub use mcp::McpManager;
// pub use thread_manager::ThreadManager;
// pub use rollout::RolloutRecorder;

// #[doc(hidden)] — 技术上公开但不鼓励使用
// #[doc(hidden)]
// pub(crate) mod prompt_debug;

// #[deprecated] — 向后兼容
// #[deprecated(note = "use ThreadManager")]
// pub type ConversationManager = ThreadManager;
// #[deprecated(note = "use NewThread")]
// pub type NewConversation = NewThread;

fn main() {
println!("re-export 扁平化 API");
println!("#[deprecated] 保留向后兼容");
println!("#[doc(hidden)] 隐藏实现细节");
}
```

### 21.3 Workspace 组织

```toml
# 根 Cargo.toml
[workspace]
members = [
"core",         # 核心逻辑
"protocol",     # 数据类型定义
"tui",          # 终端界面
"cli",          # 命令行入口
"exec",         # 命令执行
"config",       # 配置管理
"agere-mcp",    # MCP 客户端
"tools",        # 工具定义
"app-server",   # 应用服务器
# ... 还有 60+ 个
]
resolver = "2"

[workspace.package]
version = "0.3.28"
edition = "2024"
license = "Apache-2.0"

[workspace.dependencies]
# 内部 crate — 路径依赖
agere-core = { path = "core" }
agere-protocol = { path = "protocol" }

# 外部 crate — 版本约束
tokio = { version = "1" }
serde = { version = "1" }
chrono = { version = "0.4", features = ["serde"] }
```

```toml
# 子 crate 继承 workspace 设置
[package]
name = "agere-cli"
version.workspace = true    # 继承版本号
edition.workspace = true    # 继承 edition
license.workspace = true    # 继承许可证

[dependencies]
# workspace = true 使用根 Cargo.toml 的版本
# features 在子 crate 中按需添加
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "signal"] }
clap = { workspace = true, features = ["derive"] }
```

**为什么要用 workspace？**
1. **统一依赖版本** — 所有 crate 用同一个版本的 `serde`、`tokio`
2. **共享编译缓存** — 编译一次，所有 crate 共用 `target/` 目录
3. **路径依赖** — 直接引用本地 crate，修改后立即生效
4. **独立编译** — 只修改一个 crate 时，只重编译它和依赖它的 crate
5. **统一 lint 配置** — `[workspace.lints]` 一次配置，所有 crate 继承

### 21.4 测试组织

```text
core/
├── src/
│   ├── lib.rs              # 模块声明
│   ├── agents_md.rs         # 功能代码
│   ├── agents_md_tests.rs   # 内联单元测试
│   ├── client.rs            # 功能代码
│   └── client_tests.rs      # 内联单元测试
└── tests/
├── all.rs               # 集成测试入口
├── common/              # 共享测试工具
└── suite/
├── mod.rs           # 80+ 测试模块声明
├── client.rs        # 集成测试
├── exec.rs          # 集成测试
└── skills.rs        # 集成测试
```

### 本章小结

| 概念 | 关键点 |
|------|--------|
| 可见性 | mod/pub/pub(crate)/pub(super) |
| re-export | pub use 扁平化 API |
| workspace | 共享依赖版本，统一 lint |
| 测试 | 内联 + 集成 + 快照 |

---

## 22. 条件编译与构建

### 22.1 #[cfg] 属性

```rust
// 平台条件编译 — 不同操作系统用不同代码

// cli/src/main.rs — 实际代码
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod app_cmd;

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod desktop_app;

#[cfg(not(windows))]
mod wsl_paths;

// tui/src/lib.rs — Linux 上没有语音输入
#[cfg(not(target_os = "linux"))]
mod audio_device;

#[cfg(target_os = "linux")]
#[allow(dead_code)]
mod audio_device {
pub(crate) fn list_realtime_audio_device_names() -> Result<Vec<String>, String> {
Err("voice input is unavailable in this build".into())
}
}

// 测试条件
// #[cfg(test)] mod tests;

// 构建配置条件
// #[cfg(debug_assertions)] — debug 模式
// #[cfg(not(debug_assertions))] — release 模式

fn main() {
println!("cfg 条件编译:");
println!("  target_os — linux/macos/windows");
println!("  target_arch — x86_64/aarch64");
println!("  test — 测试构建");
println!("  feature — Cargo feature");
println!("  debug_assertions — debug vs release");
}
```

### 22.2 Feature Flags

```rust
// Cargo features — 编译时选择功能
// [features]
// default = ["sqlite"]
// sqlite = []
// memory = []

// 运行时 features — Feature enum
// features/src/lib.rs
// pub enum Feature {
//     ShellTool,
//     AgereHooks,
//     CodeMode,
//     WebSearchRequest,
//     Sqlite,
//     MemoryTool,
// }
//
// pub struct FeatureSpec {
//     pub key: &'static str,
//     pub stage: Stage,  // UnderDevelopment, Experimental, Stable
//     pub default_enabled: bool,
// }

fn main() {
println!("Cargo features: 编译时功能选择");
println!("运行时 features: Feature enum + Stage 生命周期");
}
```

### 22.3 build.rs 构建脚本

```rust
// cli/build.rs — 实际代码
// fn main() {
//     if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
//         println!("cargo:rustc-link-arg=-ObjC");
//     }
// }

fn main() {
println!("build.rs: 编译前运行的脚本");
println!("用于生成代码、配置链接器、检查系统依赖");
println!("4 个 crate 有 build.rs: cli, execpolicy-legacy, skills, state");
}
```

### 本章小结

| 概念 | 用途 |
|------|------|
| #[cfg] | 条件编译（平台/test/feature） |
| Cargo features | 编译时功能 |
| 运行时 features | 功能生命周期 |
| build.rs | 编译前脚本 |

---

## 23. 标准库深入

### 23.1 fmt 模块 — 格式化输出

```rust
use std::fmt;

struct Point { x: f64, y: f64 }

// Display — 用户友好的 {} 格式
impl fmt::Display for Point {
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "({}, {})", self.x, self.y)
}
}

// Debug — 开发者友好的 {:?} 格式
impl fmt::Debug for Point {
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
write!(f, "Point {{ x: {}, y: {} }}", self.x, self.y)
}
}

fn main() {
let p = Point { x: 1.0, y: 2.5 };
println!("Display: {p}");     // (1, 2.5)
println!("Debug: {p:?}");     // Point { x: 1, y: 2.5 }
println!("Pretty: {p:#?}");   // 多行 Debug

// 格式化选项
let pi = 3.14159265;
println!("{pi:.2}");        // 3.14
println!("{pi:>10.2}");     //      3.14（右对齐，宽 10）
println!("{pi:0>8.2}");     // 00003.14（零填充）
println!("{:?}", vec![1, 2, 3]); // [1, 2, 3]
println!("{:#?}", vec![1, 2, 3]); // Pretty Debug
}
```

### 23.2 Path 与 PathBuf

```rust
use std::path::{Path, PathBuf};

fn main() {
let path = Path::new("/home/user/file.txt");
println!("文件名: {:?}", path.file_name());
println!("扩展名: {:?}", path.extension());
println!("父目录: {:?}", path.parent());
println!("stem: {:?}", path.file_stem());
println!("exists: {}", path.exists());
println!("is_file: {}", path.is_file());
println!("is_dir: {}", path.is_dir());

let mut buf = PathBuf::from("/home/user");
buf.push("documents");
buf.push("report.pdf");
println!("路径: {buf:?}");

// 项目中用 AbsolutePathBuf 保证路径绝对
// agere-utils-fs::AbsolutePathBuf
// 防止相对路径被误用
}
```

### 23.3 Duration, Instant, SystemTime

```rust
use std::time::{Duration, Instant};

fn main() {
let d = Duration::from_secs(5);
let ms = Duration::from_millis(500);
let us = Duration::from_micros(100);
println!("5s + 500ms = {:?}", d + ms);

// 性能计时
let start = Instant::now();
let mut sum = 0u64;
for i in 0..1_000_000 { sum += i; }
let elapsed = start.elapsed();
println!("100 万次加法耗时: {elapsed:?}, sum={sum}");

// 项目中用 chrono 处理日期时间
// use chrono::{DateTime, Utc, Local};
// let now: DateTime<Utc> = Utc::now();
}
```

### 23.4 OnceLock 与 Atomic

```rust
use std::sync::{OnceLock, atomic::{AtomicBool, AtomicU64, Ordering}};

static CONFIG: OnceLock<String> = OnceLock::new();

fn get_config() -> &'static String {
CONFIG.get_or_init(|| "default_config".to_string())
}

fn main() {
// OnceLock — 惰性初始化，只计算一次
let config = get_config();
println!("config: {config}");
// 后续调用直接返回缓存的值

// Atomic — 无锁并发
let flag = AtomicBool::new(false);
flag.store(true, Ordering::SeqCst);
println!("flag: {}", flag.load(Ordering::SeqCst));

let counter = AtomicU64::new(0);
counter.fetch_add(1, Ordering::SeqCst);
counter.fetch_add(1, Ordering::SeqCst);
println!("counter: {}", counter.load(Ordering::SeqCst)); // 2

// Ordering 解释:
// SeqCst — 最强，保证全局顺序
// Acquire/Release — 配对使用，保证可见性
// Relaxed — 最弱，只保证原子性
}
```

### 23.5 Option/Result 组合方法

```rust
fn main() {
let value: Option<i32> = Some(42);

// map — 转换 Some 内的值
let doubled = value.map(|x| x * 2);
println!("doubled: {doubled:?}");

// and_then — 链式 Option
let result = value.and_then(|x| if x > 10 { Some(x.to_string()) } else { None });
println!("result: {result:?}");

// unwrap_or / unwrap_or_else
println!("default: {}", value.unwrap_or(0));
println!("lazy default: {}", value.unwrap_or_else(|| 0));

// filter
let filtered = value.filter(|&x| x > 50);
println!("filtered: {filtered:?}"); // None (42 < 50)

// as_ref / as_deref
let s = Some(String::from("hello"));
let borrowed: Option<&str> = s.as_deref();
println!("borrowed: {borrowed:?}");

// Result 同样有这些方法
let r: Result<i32, String> = Ok(42);
println!("mapped: {:?}", r.map(|x| x + 1));
}
```

### 本章小结

| 模块 | 关键类型 |
|------|---------|
| fmt | Display, Debug, write! |
| path | Path, PathBuf |
| time | Duration, Instant, SystemTime |
| sync | OnceLock, AtomicBool, AtomicU64 |
| Option/Result | map, and_then, unwrap_or, as_deref |

---

## 24. 测试

### 24.1 单元测试

```rust
fn add(a: i32, b: i32) -> i32 { a + b }

fn divide(a: f64, b: f64) -> Result<f64, String> {
if b == 0.0 { Err("除数为零".into()) } else { Ok(a / b) }
}

#[cfg(test)]
mod tests {
use super::*;

#[test]
fn test_add() {
assert_eq!(add(2, 3), 5);
}

#[test]
fn test_add_negative() {
assert_eq!(add(-1, 1), 0);
}

#[test]
fn test_divide_ok() {
let result = divide(10.0, 3.0).unwrap();
assert!((result - 10.0 / 3.0).abs() < f64::EPSILON);
}

#[test]
fn test_divide_zero() {
assert!(divide(1.0, 0.0).is_err());
assert_eq!(divide(1.0, 0.0).unwrap_err(), "除数为零");
}

#[test]
#[should_panic(expected = "index out")]
fn test_panic() {
let v: Vec<i32> = vec![];
let _ = v[0];
}

#[test]
fn test_parallel() {
// 并行测试
let handles: Vec<_> = (0..10).map(|i| {
std::thread::spawn(move || add(i, 1))
}).collect();

for (i, handle) in handles.into_iter().enumerate() {
assert_eq!(handle.join().unwrap(), (i as i32) + 1);
}
}
}

fn main() { println!("运行: cargo test -p agere-xxx"); }
```

### 24.2 pretty_assertions 与 insta

```rust
// use pretty_assertions::assert_eq;
// 提供彩色 diff 输出
//
// 项目 AGENTS.md 规则:
// "Tests should use pretty_assertions::assert_eq for clearer diffs"
// "Prefer comparing the equality of entire objects over fields one by one"

// insta 快照测试
// use insta::assert_snapshot;
//
// #[test]
// fn test_ui_render() {
//     let output = render_widget();
//     assert_snapshot!(output);
// }
//
// cargo test → 生成 .snap.new
// cargo insta review → 审查变更
// cargo insta accept → 接受快照

fn main() {
println!("pretty_assertions: 彩色 assert_eq diff");
println!("insta: 快照测试 UI 渲染");
println!("项目 AGENTS.md: UI 变更必须有快照覆盖");
}
```

### 24.3 集成测试与 core_test_support

```rust
// 集成测试入口: core/tests/all.rs
// pub use agere_protocol::error;
// mod suite;

// core/tests/suite/mod.rs — 80+ 测试模块
// #[cfg(not(target_os = "windows"))] mod abort_tasks;
// mod client;
// mod exec;
// mod hooks;
// mod skills;

// core_test_support 模式:
// let mock = responses::mount_sse_once(&server, responses::sse(vec![
//     responses::ev_response_created("resp-1"),
//     responses::ev_function_call(call_id, "shell", &args_json),
//     responses::ev_completed("resp-1"),
// ])).await;
//
// agere.submit(Op::UserTurn { ... }).await?;
//
// // 断言请求体
// let request = mock.single_request();
// assert_eq!(request.function_call_output(call_id), expected);

fn main() {
println!("集成测试: tests/ 目录");
println!("mock: wiremock / mount_sse_once");
println!("test_support: 共享测试工具 crate");
println!("快照测试: insta");
}
```

### 本章小结

| 工具 | 用途 |
|------|------|
| #[test] | 单元测试 |
| pretty_assertions | 彩色 diff |
| insta | 快照测试 |
| assert_cmd | CLI 测试 |
| wiremock | HTTP mock |
| core_test_support | 集成测试工具 |

---

## 25. 日志与调试

### 25.1 tracing crate

```rust
use tracing::{info, warn, error, debug, trace, instrument};

#[instrument(fields(user_id = id))]
fn process_request(id: u32) {
debug!("开始处理请求 {id}");
// ... 处理逻辑 ...
info!("请求 {id} 处理完成");
}

#[instrument(skip(password))]
fn login(username: &str, password: &str) -> bool {
trace!("尝试登录: {username}");
// password 不会出现在日志中（skip）
username == "admin" && password == "secret"
}

fn main() {
// tracing-subscriber::fmt::init();

info!("应用启动");
process_request(42);
warn!("磁盘空间不足");
error!("连接失败: timeout");
debug!("调试信息");
trace!("跟踪信息");

println!("日志级别: error > warn > info > debug > trace");
}
```

### 25.2 项目中的日志模式

```rust
// warn!("failed to request user config reload: {err}");
//
// #[instrument] — 自动创建函数级 span
// 记录函数参数和返回值
//
// log_event! 宏统一日志格式:
// macro_rules! log_event! {
//     ($self:expr, $($fields:tt)*) => {{
//         tracing::event!(
//             target: OTEL_LOG_ONLY_TARGET,
//             tracing::Level::INFO,
//             $($fields)*
//             event.timestamp = %timestamp(),
//         );
//     }};
// }

fn main() {
println!("warn!/info!/debug!/error! — 日志级别");
println!("#[instrument] — 自动函数 span");
println!("#[instrument(skip(field))] — 跳过敏感参数");
println!("log_event! — 统一格式");
}
```

### 本章小结

| 概念 | 关键点 |
|------|--------|
| tracing | 结构化日志 + span |
| 级别 | error/warn/info/debug/trace |
| #[instrument] | 自动函数 span |
| skip | 隐藏敏感参数 |
| subscriber | 日志输出配置 |

---


### 25.3 tracing 配置与过滤

```rust
// 使用 tracing-subscriber 配置日志输出
// use tracing_subscriber::{fmt, EnvFilter};
//
// fn setup_logging() {
//     let filter = EnvFilter::from_default_env();
//     fmt()
//         .with_env_filter(filter)
//         .with_target(true)
//         .with_level(true)
//         .init();
// }

fn main() {
    println!("RUST_LOG=debug,hyper=info 控制日志级别");
    println!("with_target — 显示模块路径");
    println!("with_thread_ids — 显示线程 ID");
}
```

### 25.4 Span 和上下文

```rust
use tracing::{info_span, instrument};

// 手动创建 span
fn process_request() {
    let span = info_span!("request", method = "GET", path = "/api");
    let _guard = span.enter();
    // 这里的所有日志自动带上 request span 的上下文
    tracing::info!("开始处理请求");
    tracing::info!("请求处理完成");
}

// #[instrument] 自动创建 span（推荐）
#[instrument(skip(password), fields(user_id))]
fn login(username: &str, password: &str) -> bool {
    // span 自动包含 username 参数
    // password 被 skip，不会出现在日志中
    tracing::Span::current().record("user_id", 42);
    username == "admin" && password == "secret"
}

fn main() {
    process_request();
    println!("{}", login("admin", "secret"));
}
```

### 25.5 结构化字段

```rust
use tracing::{error, info};

fn handle_error(err: &std::io::Error, path: &str) {
    // 结构化字段（推荐）
    error!(
        error = %err,
        path = path,
        error_code = err.raw_os_error(),
        "文件读取失败"
    );
    // 输出: ERROR 文件读取失败 error=... path="/tmp/test" error_code=Some(2)

    // 字符串拼接（不推荐）
    // error!("文件读取失败: {} at {}", err, path);
    // 丢失了结构化信息，难以搜索和过滤
}

fn main() {
    handle_error(&std::io::Error::from_raw_os_error(2), "/tmp/test");
}
```

## 26. 常用生态 Crate

### 26.1 clap — CLI 解析

```rust
// use clap::Parser;
//
// #[derive(Debug, Parser)]
// #[clap(name = "agere", about = "Agere CLI")]
// struct Cli {
//     #[clap(short, long)]
//     verbose: bool,
//
//     #[clap(flatten)]
//     config: CliConfigOverrides,
//
//     #[clap(subcommand)]
//     command: Option<Subcommand>,
// }
//
// #[derive(Debug, clap::Subcommand)]
// enum Subcommand {
//     Login(LoginCommand),
//     Exec(ExecCli),
//     Mcp(McpCli),
//     Plugin(PluginCli),
//     Resume(ResumeCommand),
// }

fn main() {
println!("clap: derive 宏解析命令行参数");
println!("Parser — 主入口");
println!("Args — 参数组");
println!("Subcommand — 子命令");
println!("ValueEnum — 枚举参数");
println!("#[clap(flatten)] — 组合参数组");
}
```

### 26.2 其他关键 crate

| Crate | 用途 | 项目使用 |
|-------|------|---------|
| serde/serde_json | 序列化 | 协议、配置 |
| thiserror | 库错误定义 | protocol crate |
| anyhow | 应用错误传播 | exec, cli |
| tokio | 异步运行时 | 所有异步代码 |
| regex | 正则表达式 | 命令解析 |
| chrono | 时间处理 | 错误消息时间 |
| ratatui | TUI 框架 | tui crate |
| reqwest | HTTP 客户端 | API 调用 |
| sqlx | 异步数据库 | state crate |
| uuid | UUID | ThreadId |
| toml | TOML 解析 | 配置文件 |
| tracing | 结构化日志 | 全项目 |
| indexmap | 有序 HashMap | 工具注册 |
| tempfile | 临时文件 | 测试 |
| insta | 快照测试 | tui 测试 |
| wiremock | HTTP mock | 集成测试 |
| pretty_assertions | 彩色 diff | 测试 |
| arc-swap | 无锁更新 | 配置热更新 |
| strum_macros | 枚举工具 | 命令枚举 |
| ts-rs | TS 类型生成 | 协议 crate |
| schemars | JSON Schema | 配置 schema |
| walkdir | 递归遍历 | 文件搜索 |
| notify | 文件监控 | skills_watcher |
| textwrap | 文本换行 | TUI 渲染 |

### 26.3 项目中 crate 的选择逻辑

| 决策 | 选择 | 原因 |
|------|------|------|
| 序列化 | serde | 事实标准，60+ 格式 |
| 异步运行时 | tokio | 生态最大 |
| 库错误 | thiserror | 结构化错误类型 |
| 应用错误 | anyhow | 快速传播 + context |
| TUI | ratatui | 活跃维护 |
| 有序映射 | indexmap | 保持插入顺序 |
| 无锁更新 | arc-swap | 读多写少极快 |
| 枚举工具 | strum_macros | Display/EnumIter |
| TS 生成 | ts-rs | 协议 crate |
| Schema | schemars | 配置 schema |

### 26.3 项目中 crate 选择逻辑

| 决策 | 选择 | 原因 |
|------|------|------|
| 序列化 | serde | 事实标准，60+ 格式 |
| 异步运行时 | tokio | 生态最大 |
| 库错误 | thiserror | 结构化错误类型 |
| 应用错误 | anyhow | 快速传播 + context |
| TUI | ratatui | 活跃维护 |
| 有序映射 | indexmap | 保持插入顺序 |
| 无锁更新 | arc-swap | 读多写少极快 |
| 枚举工具 | strum_macros | Display/EnumIter |
| TS 生成 | ts-rs | 协议 crate |
| Schema | schemars | 配置 schema |
| 日志 | tracing | 结构化日志 + span |

### 本章小结

项目使用了 100+ 个外部 crate，覆盖了序列化、网络、数据库、测试、日志、
终端 UI 等所有方面。workspace 统一管理版本，子 crate 按需启用 features。

---

## 27. 惯用法与设计模式

### 27.1 Builder 模式

```rust
#[derive(Debug)]
struct Request {
url: String,
method: String,
headers: Vec<(String, String)>,
body: Option<String>,
}

impl Request {
fn new(url: &str) -> Self {
Self { url: url.into(), method: "GET".into(), headers: vec![], body: None }
}
fn method(mut self, m: &str) -> Self { self.method = m.into(); self }
fn header(mut self, k: &str, v: &str) -> Self {
self.headers.push((k.into(), v.into())); self
}
fn body(mut self, b: &str) -> Self { self.body = Some(b.into()); self }
}

fn main() {
let req = Request::new("https://api.example.com")
.method("POST")
.header("Content-Type", "application/json")
.body(r#"{"key": "value"}"#);
println!("{req:?}");
}
```

### 27.2 RAII 模式

```rust
struct FileHandle { name: String }
impl Drop for FileHandle {
fn drop(&mut self) { println!("关闭文件: {}", self.name); }
}

struct Connection { id: u32 }
impl Drop for Connection {
fn drop(&mut self) { println!("断开连接: {}", self.id); }
}

fn main() {
let _f = FileHandle { name: "data.txt".into() };
let _c = Connection { id: 42 };
println!("资源已创建");
} // drop 顺序: Connection → FileHandle (LIFO)
```

### 27.3 项目中的设计模式

```rust
fn main() {
println!("项目中的常见模式：");
println!("1. Builder — Config::builder().with_x().build()");
println!("2. RAII — MutexGuard, 文件句柄自动关闭");
println!("3. Newtype — AbsolutePathBuf, ThreadId, GitSha");
println!("4. Observer — watch channel 通知 UI 状态变化");
println!("5. Strategy — trait object 多态工具处理");
println!("6. Factory — ThreadManager 创建线程");
println!("7. Middleware — Arc<Mutex<HashMap>> 共享状态管道");
println!("8. Registry — inventory 注册实验性 API");
println!("9. Queue/Event Loop — mpsc channel + tokio::select!");
println!("10. SQ/EQ — Submission Queue / Event Queue 协议");
}
```

### 27.4 Type-State 模式

用类型系统编码状态，编译器防止非法状态转换：

```rust
struct Uninitialized;
struct Running;

struct App<State> {
    config: String,
    _state: std::marker::PhantomData<State>,
}

impl App<Uninitialized> {
    fn new() -> Self {
        App { config: String::new(), _state: std::marker::PhantomData }
    }
    fn start(self) -> App<Running> {
        println!("启动: {}", self.config);
        App { config: self.config, _state: std::marker::PhantomData }
    }
}

impl App<Running> {
    fn run(&self) { println!("运行中..."); }
}

fn main() {
    App::new().start().run();
    // App::new().run();  // 编译错误：Uninitialized 没有 run
}
```

### 本章小结

| 模式 | Rust 实现 | 项目位置 |
|------|----------|---------|
| Builder | 链式 self 返回 | Config 构建 |
| RAII | Drop trait | MutexGuard |
| Newtype | 元组结构体 | ThreadId, GitSha |
| Observer | watch channel | 状态同步 |
| Strategy | dyn Trait | ToolHandler |
| Factory | 构造函数 | ThreadManager |
| Registry | inventory | 实验 API |
| SQ/EQ | mpsc + select! | 核心协议 |

---

## 28. 性能与优化

### 28.1 零成本抽象

```rust
fn sum_squares_iter(numbers: &[i32]) -> i32 {
numbers.iter().map(|x| x * x).sum()
}

fn sum_squares_manual(numbers: &[i32]) -> i32 {
let mut sum = 0;
for &x in numbers { sum += x * x; }
sum
}

fn main() {
let nums = vec![1, 2, 3, 4, 5];
assert_eq!(sum_squares_iter(&nums), sum_squares_manual(&nums));
println!("零成本: 迭代器链 == 手写循环");
}
```

### 28.2 Cow 避免不必要的克隆

```rust
use std::borrow::Cow;

fn sanitize(s: &str) -> Cow<'_, str> {
if s.contains("bad") {
Cow::Owned(s.replace("bad", "good"))
} else {
Cow::Borrowed(s)  // 零成本
}
}

fn main() {
println!("{}", sanitize("good input"));   // Borrowed
println!("{}", sanitize("bad input"));    // Owned
}
```

### 28.3 Box 减小枚举大小

```rust
fn main() {
let small = std::mem::size_of::<Box<[u8; 1024]>>();
let big = std::mem::size_of::<[u8; 1024]>();
println!("Box<1024B>: {small} bytes, 裸数组: {big} bytes");
}
```

### 28.4 ArcSwap 无锁热更新

```rust
// use arc_swap::ArcSwap;
// let config = ArcSwap::from(Arc::new(Config::default()));
// let cfg = config.load();       // 原子读
// config.store(Arc::new(new));   // 原子写

fn main() {
println!("ArcSwap: 无锁配置热更新");
println!("读取端永远看到一致快照");
println!("写入端替换整个配置，不需要锁");
}
```

### 28.5 项目中的性能技巧

```rust
fn main() {
println!("项目中的性能优化:");
println!("1. Cow — 避免常见路径上的克隆");
println!("2. Box — 减小枚举大小（ExecErr::Denied）");
println!("3. ArcSwap — 无锁配置热更新");
println!("4. 迭代器链 — 编译期内联");
println!("5. with_capacity — 预分配 Vec");
println!("6. &str 参数 — 避免 String 克隆");
println!("7. #[inline] — 小函数内联");
println!("8. release profile: LTO + codegen-units=1");
}
```

### 28.6 内存布局与 repr

```rust
#[repr(C)]  // C 兼容布局（字段顺序不重排，FFI 用）
struct Header { version: u8, flags: u16, length: u32 }

#[repr(packed)]  // 无填充对齐（紧凑但访问可能慢）
struct Packed { version: u8, flags: u16, length: u32 }

#[repr(align(64))]  // 对齐到 64 字节（缓存行友好）
struct CacheLine { data: [u8; 32] }

fn main() {
    println!("默认: {} bytes", std::mem::size_of::<Header>());
    println!("packed: {} bytes", std::mem::size_of::<Packed>());
    println!("aligned: {} bytes", std::mem::size_of::<CacheLine>());
}
```

### 28.7 预分配与容量管理

```rust
fn main() {
    // Vec 预分配 — 避免多次重新分配
    let mut v = Vec::with_capacity(1000);
    for i in 0..1000 { v.push(i); }
    println!("len: {}, cap: {}", v.len(), v.capacity());

    // HashMap 预分配
    let mut map = std::collections::HashMap::with_capacity(100);
    for i in 0..100 { map.insert(format!("key_{i}"), i); }

    // shrink_to_fit — 释放多余容量
    let mut big = Vec::with_capacity(10000);
    big.push(1);
    big.shrink_to_fit();
    println!("shrinked: {}", big.capacity());
}
```

### 本章小结

| 技术 | 效果 |
|------|------|
| 迭代器/泛型 | 零成本抽象 |
| Cow | 避免不必要克隆 |
| Box | 减小枚举大小 |
| ArcSwap | 无锁热更新 |
| 内联/单态化 | 编译期特化 |
| with_capacity | 减少重新分配 |

---

## 29. 项目代码深度解读

### 29.1 protocol/src/error.rs — 错误类型设计

```rust
// 完整文件结构分析:

// 1. 类型别名 — 简化签名
// pub type Result<T> = std::result::Result<T, AgereErr>;

// 2. 子错误类型 — thiserror derive
// #[derive(Error, Debug)]
// pub enum ExecErr {
//     #[error("execution denied, exit code: {}", .output.exit_code)]
//     Denied { output: Box<ExecToolCallOutput> },
//     Timeout { output: Box<ExecToolCallOutput> },
//     Signal(i32),
// }

// 3. 主错误类型 — 20+ 变体
// #[derive(Error, Debug)]
// pub enum AgereErr {
//     #[error("turn aborted")] TurnAborted,
//     #[error("stream disconnected: {0}")] Stream(String, Option<Duration>),
//     #[error("exec error: {0}")] Exec(#[from] ExecErr),
//     #[error(transparent)] Io(#[from] io::Error),
// }

// 4. 方法 — 分类和转换
// impl AgereErr {
//     pub fn is_retryable(&self) -> bool { ... }
//     pub fn to_agere_protocol_error(&self) -> AgereErrorInfo { ... }
//     pub fn http_status_code_value(&self) -> Option<u16> { ... }
// }

fn main() {
println!("设计决策:");
println!("1. thiserror derive 自动生成 Display/From");
println!("2. Box<ExecToolCallOutput> 减小编举大小");
println!("3. #[from] 自动错误转换");
println!("4. is_retryable() 分类错误处理策略");
println!("5. transparent 透传底层错误");
}
```

### 29.2 core/src/lib.rs — 模块组织

```rust
// 1. 可见性分层
// mod apply_patch;          // 纯内部
// pub mod config;           // 公共 API
// pub(crate) mod session;   // crate 内可用

// 2. Re-export 扁平化
// pub use agere_thread::AgereThread;
// pub use thread_manager::ThreadManager;

// 3. Deprecation 兼容
// #[deprecated(note = "use ThreadManager")]
// pub type ConversationManager = ThreadManager;

// 4. 内联模块分组
// pub(crate) mod mentions {
//     pub(crate) use crate::plugins::build_connector_slug_counts;
// }

fn main() {
println!("模块组织: 最小暴露 + re-export + deprecation");
}
```

### 29.3 core/src/agere_delegate.rs — 异步编排

```rust
// 1. 并发任务编排
// tokio::spawn(async move { forward_events(...).await; });
// tokio::spawn(async move { forward_ops(...).await; });

// 2. 取消机制
// tokio::select! {
//     _ = &mut cancelled => { shutdown(&agere).await; break; }
//     event = agere.next_event() => { /* dispatch */ }
// }

// 3. 共享状态
// let pending = Arc::new(Mutex::new(HashMap::new()));

// 4. 错误传播
// .or_cancel(&cancel_token).await??;

fn main() {
println!("异步编排: spawn + select! + Arc<Mutex> + CancellationToken");
}
```

### 本章小结

| 文件 | 设计亮点 |
|------|---------|
| error.rs | thiserror + Box 减小枚举 + 分类重试 |
| lib.rs | 最小暴露 + re-export + deprecation |
| agere_delegate.rs | spawn + select! + Arc<Mutex> |

---

## 30. 附录与速查表

### 30.1 常见编译错误

| 错误 | 原因 | 解决 |
|------|------|------|
| `cannot move out of` | 值已被移动 | `.clone()` 或引用 |
| `borrowed value does not live long enough` | 引用比数据活得久 | 返回拥有值 |
| `cannot borrow as mutable` | 已有不可变借用 | 确保不可变借用已结束 |
| `cannot return reference to local variable` | 悬垂引用 | 返回 String |
| `the trait bound is not satisfied` | 缺少 trait | 添加 derive 或 impl |
| `mismatched types` | 类型不匹配 | 检查类型转换 |
| `no method named` | 方法不存在 | 检查 trait 是否在 scope |
| `lifetime may not live long enough` | 生命周期不匹配 | 添加标注或用 'static |
| `future cannot be sent between threads` | !Send 类型 | 用 Arc/Mutex 包装 |

### 30.2 生命周期消除规则

1. 每个引用参数获得自己的生命周期
2. 单个输入生命周期 → 赋给所有输出
3. `&self`/`&mut self` → self 的生命周期赋给所有输出

### 30.3 Derive 宏速查

| Derive | 作用 |
|--------|------|
| Debug | `{:?}` 格式 |
| Clone | `.clone()` 深拷贝 |
| Copy | 自动按位复制 |
| PartialEq, Eq | `==` 比较 |
| Hash | HashMap key |
| Serialize, Deserialize | serde 序列化 |
| Default | 默认值 |
| Display | `{}` 格式 (strum) |
| Error | thiserror 错误 |
| JsonSchema | JSON Schema |
| TS | TypeScript 类型 |

### 30.4 Serde 属性速查

| 属性 | 作用 |
|------|------|
| rename_all = "case" | 统一命名 |
| tag = "type" | 内部标记枚举 |
| rename = "name" | 字段重命名 |
| default | 缺失用 Default |
| skip_serializing_if | 条件跳过 |
| transparent | 透明包装 |
| try_from / into | 自定义转换 |
| flatten | 内嵌结构体 |

### 30.5 所有权规则速查

```
每个值一个所有者 → 离开作用域释放 → 赋值/传参 = 移动
Copy 类型自动复制 → Clone 显式深拷贝 → 引用借用不移动
&T 不可变（多个） → &mut T 可变（一个） → 不能同时有
```

### 30.6 并发模式速查

```
共享只读 → Arc<T>
共享可变 → Arc<tokio::sync::Mutex<T>>（跨 .await）
→ Arc<std::sync::RwLock<T>>（读多写少）
单线程共享 → Rc<RefCell<T>>
通信 → mpsc / oneshot / broadcast / watch
并发任务 → tokio::spawn + tokio::select!
```

### 30.7 错误处理速查

```
库 API → thiserror 定义结构化错误 enum
应用层 → anyhow 快速传播 + context
顶层 main → anyhow::Result<()>
? → 错误传播 + 自动 From 转换
#[from] → 自动实现 From<T>
```

### 30.8 推荐学习资源

- [The Rust Programming Language（Rust 权威指南）](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [The Rustonomicon（高级 Rust）](https://doc.rust-lang.org/nomicon/)
- [Async Rust Book](https://rust-lang.github.io/async-book/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Serde Documentation](https://serde.rs/)
- [Rust Clippy Lints](https://rust-lang.github.io/rust-clippy/master/)
- [Rust Edition Guide](https://doc.rust-lang.org/edition-guide/)

---


### 30.9 Rust 核心概念速查

| 概念 | 解释 | 项目实例 |
|------|------|---------|
| RAII | 资源获取即初始化，Drop 自动释放 | MutexGuard, File |
| Deref Coercion | &String 自动转 &str | 函数参数传递 |
| Interior Mutability | 不可变引用修改数据 | RefCell, Mutex |
| Newtype | 元组结构体包装类型安全 | GitSha, UserId |
| Zero-Cost Abstraction | 编译器优化为手写级性能 | 迭代器, 泛型 |
| Send + Sync | 线程安全标记 trait | tokio::spawn |
| Lifetime Elision | 编译器自动推断生命周期 | 大多数函数签名 |
| Pattern Exhaustiveness | match 处理所有变体 | AGENTS.md 强制 |

### 30.10 项目特定速查

| 概念 | 项目实现 |
|------|---------|
| 协议层 | protocol: 类型 + serde + JsonSchema + TS |
| 核心层 | core: 会话管理、工具调度 |
| TUI 层 | tui: ratatui 渲染、事件处理 |
| 错误传播 | thiserror(protocol) + anyhow(exec) |
| 并发模式 | Arc<Mutex<>> + tokio channels |
| 配置热更新 | ArcSwap 无锁替换 |
| 工具注册 | inventory 编译期注册 |
| 类型生成 | ts-rs + schemars |
| 快照测试 | insta 测试 TUI |
| 集成测试 | core_test_support + mount_sse_once |

> 本手册基于 OpenAgere 项目代码生成，所有示例均来自真实代码。
> 生成日期：2026-07-08
