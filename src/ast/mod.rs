//! AST：块与节点的类型定义。
//!
//! 这一层是**渲染器与调度器唯一依赖的稳定形状**，也是唯一对外暴露数据结构
//! 的层；解析器的实现细节不在这里。依赖方向固定为 `parse → ast`、
//! `dispatch → ast`、`html → ast`，`ast` 永不反向依赖。
//!
//! 两层树：
//!
//! * [`Block`]：**语法层**，`parse` 的输出，只描述切分与识别的结果；
//! * [`Node`]：**展开/渲染层**，树里可以同时存在未展开节点（[`Node::Call`]）
//!   与已展开节点。

mod block;
mod node;
mod params;
mod span;

pub use block::{Block, CallBlock, NaturalBlock};
pub use node::{Attr, Element, ErrorKind, ErrorNode, Node};
pub use params::Params;
pub use span::Span;
