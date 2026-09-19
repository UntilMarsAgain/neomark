//! # neomark
//!
//! neomark 是一种“一切皆块”的标记语言。块分两类：
//!
//! * **自然块**：由书写自然产生，空行是切分标志；
//! * **调用块**：由 `::name 参数...` 调用模板/脚本产生，缩进界定块体。
//!
//! ## 流水线
//!
//! ```text
//! 源文本 ──parse_blocks──▶ Vec<Block> ──Dispatcher──▶ Vec<Node> ──渲染器──▶ HTML
//!         语法层              语法树          展开/渲染树
//! ```
//!
//! 解析与展开是**两个阶段**：`parse_blocks` 只做切分与识别，不解释任何块的
//! 语义；调用块由注册的 [`Handler`] 展开成 [`Node`]，展开过程中未展开的子树
//! 保持语法形态。
//!
//! ## 模块结构
//!
//! * [`ast`]：块与节点的数据形状，是调度器和渲染器唯一依赖的稳定层；
//! * [`dispatch`]：调用名 → 展开器的分发与改写；
//! * `parse`：解析器实现，内部模块不对外暴露，只通过 crate 根重新导出入口。
//!
//! 依赖方向固定为 `parse → ast`、`dispatch → ast`、`html → ast`；
//! `ast` 永不反向依赖。
//!
//! ## 用法
//!
//! ```
//! use neomark::{Context, Dispatcher, Registry, parse_blocks};
//!
//! let source = "::notice type=warning:\n  小心！";
//! let blocks = parse_blocks(source);
//!
//! // 没有展开器认领的块会变成 Node::Error（叶子，带着原文内容），
//! // 而不是让解析或展开失败。
//! let dispatcher = Dispatcher::new(Registry::new());
//! let mut ctx = Context::new(source);
//! let nodes = dispatcher.run(blocks, &mut ctx);
//!
//! assert_eq!(nodes.len(), 1);
//! ```
//!
//! ## 切分规则
//!
//! 1. 任何一行（忽略前导缩进）以 `::` 开头，就是调用块的头部行，
//!    即使它前面没有空行。
//! 2. 头部行之后，缩进**严格大于**头部行缩进的行属于该调用块的块体。
//! 3. 缩进一旦不再严格大于，该行就是下一个块的首行，末尾无需空行。
//! 4. 调用块体内的空行不结束块（只要后面还有更深缩进的行），
//!    末尾的连续空行不算内容。
//! 5. 块体去掉公共缩进后递归解析，因此调用块可以嵌套。

pub mod ast;
pub mod dispatch;

mod parse;

pub use ast::{
    Attr, Block, CallBlock, Element, ErrorKind, ErrorNode, NaturalBlock, Node, Params, Span,
};
pub use dispatch::{Context, Dispatcher, Fallback, Handler, Registry};
pub use parse::{CallHeader, parse_blocks, parse_call_header};
