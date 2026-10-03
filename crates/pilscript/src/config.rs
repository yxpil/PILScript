//! 解释器能力配置：按最小权限原则裁剪内建能力。

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Config {
    /// 允许 dlopen 加载外部动态库（FFI）
    pub allow_ffi: bool,
    /// 允许读写文件系统（read_file/write_file/append_file/exists）
    pub allow_fs: bool,
}

impl Default for Config {
    /// 默认全部开放（保持 v0.1 交互体验）；嵌入方与 CLI 可按需收紧
    fn default() -> Self {
        Config { allow_ffi: true, allow_fs: true }
    }
}
