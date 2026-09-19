//! AST：块的类型定义。
//!
//! 这一层是**渲染器唯一依赖的稳定形状**，也是唯一对外暴露数据结构的层；
//! 解析器的实现细节不在这里。依赖方向固定为 `parse → ast`、`html → ast`，
//! `ast` 永不依赖 `parse`。

mod block;
mod params;

pub use block::{Block, CallBlock, NaturalBlock};
pub use params::Params;
