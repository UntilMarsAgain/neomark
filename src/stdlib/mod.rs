//! 调用块**标准库**：内置的调用块展开器。
//!
//! 这里只放「展开器本身」。它们用到的机制（[`Wrap`](crate::handlers::Wrap)、
//! [`NaturalExpander`]）在
//! [`handlers`](crate::handlers) 里，与标准库分开——机制会随内核演进，标准库只会
//! 越来越长。
//!
//! 每个展开器一个文件、一个 `register` 函数；[`register_defaults`] 把它们一次装上。
//! 只要一部分就自己挑：
//!
//! ```
//! use neomark::{Registry, handlers::NaturalExpander, stdlib};
//!
//! let mut registry = Registry::new();
//! registry.register_natural(NaturalExpander::default());
//! stdlib::code::register(&mut registry);   // 只要代码块，不要标题和引用
//! ```
//!
//! # 现有的
//!
//! | 调用 | 产出 | 备注 |
//! |---|---|---|
//! | `::h1` ~ `::h6` | `<h1 class="nm-h1">` | 一条正则模式注册的包装器 |
//! | `::quote origin=…` | `<blockquote class="nm-quote" data-origin="…">` | 块体按块级解析 |
//! | `::code language=…` | `<pre class="nm-code-block"><code class="language-…">` | 块体**原样**输出 |
//!
//! 认领不了的调用名依然落到兜底展开器（报错节点），不会被悄悄吞掉。

pub mod code;
pub mod heading;
pub mod quote;

use crate::dispatch::Registry;
use crate::handlers::NaturalExpander;

/// 注册全部内置展开器：自然块槽位 + 标准库里的各个调用块。
///
/// 想要自己的版本（比如换一套行内配置、不要某个展开器）就照这里的写法自己拼。
pub fn register_defaults(registry: &mut Registry) {
    let natural = NaturalExpander::default();

    registry.register_natural(natural.clone());

    heading::register(registry, natural);
    quote::register(registry);
    code::register(registry);
}
