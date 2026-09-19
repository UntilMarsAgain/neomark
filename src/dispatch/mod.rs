//! 调用块的分发与展开。
//!
//! 这是一个**改写系统**：树里可以同时存在未展开节点（[`Node::Call`]）与
//! 已展开节点，[`Dispatcher`] 按调用名把前者交给 [`Registry`] 里注册的
//! [`Handler`]，直到树中不再有未展开节点。
//!
//! # 展开是自上而下的
//!
//! 展开器返回的子树决定了它的子节点会被访问到什么程度：把 `call.body`
//! 放进子树，子节点才会被继续展开；像 `::code` 那样丢掉 `call.body`、
//! 改用 `call.raw_body`，内层就永远不会被访问——**不需要解析器特判**。
//!
//! # 报错是数据，不是控制流
//!
//! 没有注册展开器时，调度器产出 [`Node::Error`]，它是**叶子**。怎么显示
//! 由渲染器决定，整个文档不会因为一个块出错而渲染不出来。

mod dispatcher;
mod handler;
mod registry;

pub use dispatcher::Dispatcher;
pub use handler::{Context, Handler};
pub use registry::Registry;
