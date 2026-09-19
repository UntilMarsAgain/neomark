//! 块的分发与展开。
//!
//! 这是一个**改写系统**：一棵 `arena` 树里可以同时存在未展开节点与已展开
//! 节点，[`Dispatcher`] 按块的种类把前者交给 [`Registry`] 里注册的
//! [`Handler`]，直到树中不再有未展开节点。
//!
//! # 自然块没有超然待遇
//!
//! 自然块和调用块都是未展开的块，走同一条 `查找 → 应用 → 递归` 路径：
//! 调用块按调用名查找，自然块查找自然块展开器；**两者都没命中时用兜底
//! 展开器** [`Fallback`]，它把块的原文内容放进报错节点。
//!
//! # 展开器拿的是节点 id
//!
//! ```
//! use neomark::{Ast, Attr, Context, Handler, NodeId};
//!
//! struct Notice;
//!
//! impl Handler for Notice {
//!     fn expand_call(
//!         &self,
//!         node: NodeId,
//!         ast: &mut Ast,
//!         _ctx: &mut Context<'_>,
//!     ) -> Vec<NodeId> {
//!         let div = ast.new_element("div", vec![Attr::new("class", "notice")]);
//!         // 移交块体：一次 append 就把原有子节点搬过来了
//!         for child in ast.children(node).collect::<Vec<_>>() {
//!             ast.append(div, child);
//!         }
//!         vec![div]
//!     }
//! }
//! ```
//!
//! 展开是自上而下的：展开器返回的子树决定了它原有的子节点会被访问到什么
//! 程度。像 `::code` 那样**不搬运**子节点，内层就永远不会被展开——调度器
//! 会连同原节点一起删掉它们，**不需要解析器特判**。
//!
//! # 报错是数据，不是控制流
//!
//! 没有展开器认领时产出 [`crate::ast::NodeKind::Error`]，它是**叶子**，且
//! 已经带着块的原文内容。怎么显示由渲染器决定，整个文档不会因为一个块
//! 出错而渲染不出来。

mod dispatcher;
mod handler;
mod registry;

pub use dispatcher::Dispatcher;
pub use handler::{Context, Fallback, Handler};
pub use registry::Registry;
