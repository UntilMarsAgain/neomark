//! 内置展开器。
//!
//! 展开器只依赖 [`crate::ast`]，产出的是**元素节点**而不是 HTML 字符串：
//! 真正把标签写成文本是渲染器的事。不过标签名与 class 名确实是为 HTML
//! 挑的——neomark 只以 HTML 为目标。

mod natural;

pub use natural::ParagraphHandler;

use crate::dispatch::Registry;

/// 注册全部内置展开器。
///
/// 目前只有自然块展开器；调用块（`::code` / `::notice` / `::image` /
/// `::quote` …）尚未实现，会走兜底展开器变成报错节点。
///
/// ```
/// use neomark::{Context, Dispatcher, Registry, handlers, parse};
///
/// let source = "一段普通文字。";
/// let mut ast = parse(source);
/// let mut registry = Registry::new();
/// handlers::register_defaults(&mut registry);
///
/// let mut ctx = Context::new(source);
/// Dispatcher::new(registry).run(&mut ast, &mut ctx);
///
/// assert_eq!(neomark::html::render(&ast), "<p class=\"nm-p\">一段普通文字。</p>");
/// ```
pub fn register_defaults(registry: &mut Registry) {
    registry.register_natural(ParagraphHandler);
}
