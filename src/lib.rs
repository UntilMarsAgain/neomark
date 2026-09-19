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
//! 源文本 ──parse──▶ Ast（arena 树） ──Dispatcher──▶ Ast（已展开）──▶ HTML
//!                   一棵树同时容纳未展开与已展开节点
//! ```
//!
//! 解析与展开是**两个阶段**：`parse` 只做切分与识别，不解释任何块的语义；
//! 调用块由注册的 [`Handler`] 展开成元素节点。
//!
//! ## 模块结构
//!
//! * [`ast`]：块树 —— 一棵 `indextree` arena 树，是调度器与渲染器唯一依赖的稳定层；
//! * [`dispatch`]：块的种类 → 展开器的分发与改写；
//! * [`handlers`]：内置展开器（目前只有自然块 → `<p>`）；
//! * [`html`]：把已展开的树写成 HTML，[`html::render_page`] 产出内嵌默认 CSS 的完整页面；
//! * `parse`：解析器实现，内部模块不对外暴露，只通过 crate 根重新导出入口。
//!
//! 依赖方向固定为 `parse → ast`、`dispatch → ast`、`handlers → ast`、
//! `html → ast`；`ast` 永不反向依赖。
//!
//! ## 用法
//!
//! ```
//! use neomark::{Context, Dispatcher, Registry, handlers, html, parse};
//!
//! let source = "一段普通文字。";
//! let mut ast = parse(source);
//!
//! let mut registry = Registry::new();
//! handlers::register_defaults(&mut registry);
//!
//! let mut ctx = Context::new(source);
//! Dispatcher::new(registry).run(&mut ast, &mut ctx);
//!
//! assert_eq!(html::render(&ast), "<p class=\"nm-p\">一段普通文字。</p>");
//! ```
//!
//! 没有展开器认领的块会变成报错节点（叶子，带着原文内容），渲染器把它画成
//! 信息提示框，而不是让解析或展开失败。
//!
//! ## 切分规则
//!
//! 1. 任何一行（忽略前导缩进）以 `::` 开头，就是调用块的头部行，
//!    即使它前面没有空行。
//! 2. 头部行之后，缩进**严格大于**头部行缩进的行属于该调用块的块体。
//! 3. 缩进一旦不再严格大于，该行就是下一个块的首行，末尾无需空行。
//! 4. 调用块体内的空行不结束块（只要后面还有更深缩进的行），
//!    末尾的连续空行不算内容。
//! 5. 块体去掉公共缩进后递归解析，块体就是该调用节点的子节点。
//! 6. 头部行的**分隔冒号**之后的内容是块体的**第一行**：
//!    `::notice type=warning: 小心！` 的块体就是 `小心！`。首行内容的缩进
//!    视为 0，所以续行以 0 为基准衡量、相对缩进被原样保留。

pub mod ast;
pub mod dispatch;
pub mod handlers;
pub mod html;
pub mod inline;

mod parse;

/// 底层树库。`Ast::arena()` 暴露的就是它的 [`indextree::Arena`]。
pub use indextree;

pub use ast::{
    Ast, Attr, Block, CallBlock, ErrorKind, ErrorNode, KindTag, NaturalBlock, NodeId, NodeKind,
    Params, Span,
};
pub use dispatch::{Context, Dispatcher, Fallback, Handler, Registry};
pub use parse::{CallHeader, parse, parse_call_header};
