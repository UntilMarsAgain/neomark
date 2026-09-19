//! 解析器实现：行扫描 → 头部识别 → 切分与递归。
//!
//! 这一层是内部实现，可以自由重构。模块本身不对外暴露，
//! 真正需要公开的入口由 crate 根重新导出（`neomark::parse_blocks` 等）。

mod header;
mod line;
mod split;

pub use header::{CallHeader, parse_call_header};
pub use split::parse_blocks;
