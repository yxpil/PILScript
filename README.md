# PILScript

<div align="center">

**极简 JS 风格脚本语言 · Rust 实现 · 原生跨语言 FFI**

`pilscript run your.pil` —— 就这么简单

</div>

---

PILScript 是一门参考 JavaScript 语法、刻意做减法的脚本语言：

- ✅ 变量、函数、闭包、递归、数组、对象
- ✅ **跨语言调用**：一行 `dlopen` 直接加载 C / C++ / Rust / Zig / Nim / Go(cgo) 等任何导出 C ABI 的动态库
- ✅ Go 风格自动分号：行尾是完整语句就自动结束，行尾是运算符则自动续行
- ❌ 没有 class、async、原型链、类型强转、`undefined` vs `null` 之争 —— 不整花活

解释器约 1500 行 Rust，零宏魔法（FFI 分发宏除外），适合作为学习"如何用 Rust 写解释器"的参考项目。

## 快速开始

```bash
cargo build --release
./target/release/pilscript run examples/basics.pil
./target/release/pilscript repl
```

## 语法速览

```javascript
// 变量（分号可写可不写，Go 风格自动分号）
let x = 10
let name = "PILScript"

// 函数（支持递归、闭包、一等公民）
function fib(n) {
    if (n < 2) { return n }
    return fib(n - 1) + fib(n - 2)
}

function counter() {
    let n = 0
    return function() { n += 1; return n }
}

// 数组与对象
let arr = [1, 2, 3]
push(arr, 4)
arr.length          // 4
arr[-1]             // 4（支持负索引）

let user = { name: "Ada", age: 36 }
user.lang = "math"
keys(user)          // ["name", "age", "lang"]

// 控制流
for (let i = 0; i < 10; i += 1) { print(i) }
while (x > 0) { x -= 1 }

// 值类型：null / bool / num / str / array / object / function
// 没有 undefined，没有类型强转，== 是严格相等
```

## 内置函数

| 函数 | 说明 |
|---|---|
| `print(...)` | 打印（空格分隔） |
| `len(x)` | 字符串/数组/对象长度 |
| `str(x)` / `num(x)` / `type(x)` | 类型转换与查询 |
| `push(arr, v)` / `pop(arr)` | 数组操作 |
| `keys(obj)` | 对象键列表 |
| `abs` `floor` `ceil` `sqrt` `min` `max` | 数学函数 |
| `assert(cond, msg?)` | 断言 |
| `dlopen(path)` | **加载动态库，开启跨语言调用** |

## 跨语言 FFI 🔥

任何能导出 **C ABI** 动态库的语言都能被 PILScript 调用：

```javascript
// 加载动态库（.dll / .so / .dylib）
let lib = dlopen("target/debug/demo_lib.dll");

// 按签名绑定符号：lib.fn("符号名", [参数类型], "返回类型")
let add = lib.fn("add_i64", ["i64", "i64"], "i64");
let shout = lib.fn("shout", ["str"], "str");
let even = lib.fn("is_even", ["i64"], "bool");

add(20, 22)          // 42
shout("hello")       // "HELLO"
even(42)             // true

// FFI 函数是一等公民，可以随意传递
function apply(f, a, b) { return f(a, b) }
apply(add, 1, 2)     // 3
```

**支持的参数类型**：`i32` `i64` `f64` `bool` `str` `ptr`
**支持的返回类型**：`void` `i32` `i64` `f32` `f64` `bool` `str` `ptr`

### 各语言导出示例

<details>
<summary><b>Rust（cdylib）</b></summary>

```rust
// crates/demo_lib 就是现成的例子，cargo build 后直接 dlopen
#[no_mangle]
pub extern "C" fn add_i64(a: i64, b: i64) -> i64 { a + b }
```
</details>

<details>
<summary><b>C / C++</b></summary>

```c
// gcc -shared -o math.dll math.c   (Windows)
// gcc -shared -fPIC -o libmath.so math.c  (Linux)
int add_i64(int a, int b) { return a + b; }
```
</details>

<details>
<summary><b>Zig</b></summary>

```zig
// zig build-lib -dynamic math.zig
export fn add_i64(a: i64, b: i64) i64 { return a + b; }
```
</details>

<details>
<summary><b>Go（cgo export）</b></summary>

```go
// go build -buildmode=c-shared -o math.dll main.go
import "C"

//export add_i64
func add_i64(a, b C.int64_t) C.int64_t { return a + b }
```
</details>

### FFI 实现说明

参数在内部统一为两类"槽位"：整数族（`i32`/`i64`/`bool`/`str`/`ptr`）走 `i64` 槽，
浮点走 `f64` 槽。解释器用声明式宏为**元数 0~5 的全部槽位组合**（2⁰+…+2⁵ = 63 种）
生成编译期确定的 `extern "C"` 函数指针并转译调用，返回值按 8 种类型分别读取
（含 C 字符串的安全转换与 NULL → `null` 映射）。超过 5 个参数的签名会在运行期得到清晰报错。

## 项目结构

```
crates/
├── pilscript/          解释器
│   ├── src/lexer.rs    词法分析
│   ├── src/parser.rs   递归下降解析
│   ├── src/interp.rs   树遍历求值器（作用域/闭包/控制流）
│   ├── src/foreign.rs  FFI：libloading + 签名分发宏
│   └── src/builtins.rs 内置函数库
└── demo_lib/           Rust cdylib 演示库（FFI 测试用）
examples/               示例脚本
tests → crates/pilscript/tests/
```

## 设计取舍

| 舍弃 | 原因 |
|---|---|
| `var` / `const` | 只有 `let`，一个就够 |
| `class` / 原型 | 对象字面量 + 闭包已覆盖绝大多数场景 |
| `==` 类型强转 | 严格相等，杜绝隐式转换陷阱 |
| `undefined` | 只有 `null`，越界访问返回 `null` |
| 异步 / Promise | 解释器是单线程同步模型 |
| 模板字符串 | `+` 拼接字符串足够直观 |

## 开发

```bash
cargo test          # 15 个集成测试（含 FFI 跨语言调用）
cargo clippy --all-targets
```

## License

MIT
