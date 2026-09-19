//! 内置展开器。
//!
//! 目前提供：
//!
//! * [`NaturalExpander`]——自然块 → 段落（或行内内容）。它既是一个
//!   [`Handler`](crate::dispatch::Handler)，也是一个**可直接调用的接口**，
//!   其他展开器解析文本时用它。
//! * [`Headings`]——`::h1` ~ `::h6`，用一条通配模式注册。
//!
//! 注册默认展开器用 [`register_defaults`]。想换行内配置，就自己构造一个
//! [`NaturalExpander`] 再注册。

mod headings;
mod natural;

pub use headings::Headings;
pub use natural::NaturalExpander;

use crate::dispatch::Registry;

/// 注册内置展开器。
///
/// * 自然块槽位：默认配置的 [`NaturalExpander`]；
/// * `::h1` ~ `::h6`：一条通配模式 `h?`，共用同一个 [`Headings`]。
pub fn register_defaults(registry: &mut Registry) {
    let natural = NaturalExpander::default();

    registry.register_natural(natural.clone());
    registry.register_pattern("h?", Headings::new(natural));
}
