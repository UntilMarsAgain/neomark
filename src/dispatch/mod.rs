//! 块的分发与展开。
//!
//! 这是一个**改写系统**：树里可以同时存在未展开节点（[`Node::Call`] /
//! [`Node::Natural`]）与已展开节点，[`Dispatcher`] 按块的类型把前者交给
//! [`Registry`] 里注册的 [`Handler`]，直到树中不再有未展开节点。
//!
//! # 自然块没有超然待遇
//!
//! 自然块和调用块都是未展开的块，走同一条 `查找 → 应用 → 递归` 路径：
//! 调用块按调用名查找，自然块查找自然块展开器；**两者都没命中时用兜底
//! 展开器** [`Fallback`]，它把块的原文内容放进报错节点。
//!
//! # 展开是自上而下的
//!
//! 展开器返回的子树决定了它的子节点会被访问到什么程度：把 `call.body`
//! 放进子树，子节点才会被继续展开；像 `::code` 那样丢掉 `call.body`、
//! 改用 `call.raw_body`，内层就永远不会被访问——**不需要解析器特判**。
//!
//! # 报错是数据，不是控制流
//!
//! 没有展开器认领时产出 [`Node::Error`]，它是**叶子**，且已经带着块的原文
//! 内容。怎么显示由渲染器决定，整个文档不会因为一个块出错而渲染不出来。

mod dispatcher;
mod handler;
mod registry;

pub use dispatcher::Dispatcher;
pub use handler::{Context, Fallback, Handler};
pub use registry::Registry;
