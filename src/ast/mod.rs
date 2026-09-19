//! AST：块树的类型定义。
//!
//! 这一层是**解析器、调度器与渲染器唯一依赖的稳定形状**，也是唯一对外暴露
//! 数据结构的层。依赖方向固定为 `parse → ast`、`dispatch → ast`、
//! `html → ast`，`ast` 永不反向依赖。
//!
//! # 一棵树，而不是两棵
//!
//! 整棵树保存在 [`Ast`] 的 `indextree::Arena` 里，**一棵树同时容纳未展开
//! 节点与已展开节点**：
//!
//! ```text
//! NodeKind::Document            // 唯一根
//!   ├── Natural / Call          // 未展开
//!   ├── Element / Text / Error  // 已展开
//!   └── ...
//! ```
//!
//! 孩子关系完全由 arena 的边表示，[`NodeKind`] 里没有任何 `children` 字段。

mod block;
mod error;
mod params;
mod span;
mod tree;

#[cfg(test)]
pub(crate) mod test_util;

pub use block::{Block, CallBlock, NaturalBlock};
pub use error::{ErrorKind, ErrorNode};
pub use indextree::NodeId;
pub use params::Params;
pub use span::Span;
pub use tree::{Ast, Attr, KindTag, NodeKind};
