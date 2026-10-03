# 测试说明（PILScript）

PILScript 是一个脚本语言解释器（lexer → parser → interpreter）。测试全部是
`crates/pilscript/tests/` 下的黑盒集成测试，通过 `run_source` / `run_source_with`
执行脚本并断言行为。

## 怎么跑

```powershell
# 1) 先构建一次，产出 FFI 测试需要的 demo_lib.dll（cdylib）
cargo build

# 2) 再跑全部测试
cargo test

# 只跑某一类
cargo test --test lang_tests
cargo test --test injection_tests      # 注入/数据即数据
cargo test --test hook_lifecycle_tests # FFI 回调（嵌入钩子）生命周期
```

> 注意：FFI 相关测试（`ffi_tests`、`hook_lifecycle_tests`、`injection_tests` 里的
> NUL 用例）依赖 `target/debug/demo_lib.dll`。若直接 `cargo test` 报
> “demo_lib 未构建”，先执行一次 `cargo build`。

## 预期结果

全部通过，0 失败：

| 测试文件 | 数量 | 覆盖 |
|----------|------|------|
| `lang_tests.rs` | 12 | 算术/作用域/闭包/递归/循环/数组对象/短路/错误 |
| `persistence_tests.rs` | 4 | 文件读写往返、`allow_fs`/`allow_ffi` 权限拒绝、`time()` |
| `ffi_tests.rs` | 3 | FFI 类型组合、一等公民、错误上报 |
| `stress_tests.rs` | 3 | 16 线程并发隔离、错误隔离、深度限制 |
| `injection_tests.rs` | 4 | **注入：数据即数据**（新增） |
| `hook_lifecycle_tests.rs` | 3 | **嵌入钩子（FFI）生命周期**（新增） |

## 注入测试（injection_tests.rs）

PILScript 把脚本当不可信输入执行。断言：

- `xss_and_sqli_payloads_are_opaque_string_data`：含 `<script>`、`' OR '1'='1'--`、
  `require(...)` 的字符串作为文件内容**逐字节往返**，不被求值/执行。
- `script_inside_a_string_is_not_evaluated`：字符串里写着 `assert(false)` /
  `function evil(){}` 也只是文本，不触发。
- `ffi_string_argument_with_nul_byte_is_rejected`：FFI 字符串参数里的 `\0` 被
  CString 防护拒绝（防 C 字符串截断注入）。
- `fs_path_type_confusion_is_a_clean_error_not_panic`：把数字/数组当文件路径得到
  结构化运行时错误，不 panic。

## 钩子生命周期测试（hook_lifecycle_tests.rs）

PILScript 没有事件总线；它的“钩子/回调”机制是嵌入方把外部动态库函数**注册**进来
（`dlopen` → `lib.fn(...)`），再像普通函数一样**触发**。覆盖：

- `register_bind_then_invoke_passes_args_in_position_order`：注册后触发，参数按
  声明位置传递（用混合槽位 `mix(i, f, i) = a + b*c` 验证顺序敏感）。
- `unregistered_symbol_and_disabled_ffi_are_rejected`：未注册符号拒绝；`allow_ffi=false`
  时 `dlopen` 入口本身被拒（越权拒绝）。
- `one_broken_embedding_does_not_crash_the_next`：一个会话里脚本/FFI 出错
  （深度溢出）被 catch_unwind 隔离，另一个全新会话照常运行（失败隔离）。
