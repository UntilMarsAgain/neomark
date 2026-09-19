//! 内置展开器。
//!
//! 目前提供：
//!
//! * [`NaturalExpander`]——自然块 → 段落（或行内内容）。它既是一个
//!   [`Handler`](crate::dispatch::Handler)，也是一个**可直接调用的接口**，
//!   其他展开器解析文本时用它。
//! * [`Wrap`]——**注册时的语法糖**：把调用套一层标签/带 class 的 div。
//!   内置的 `::h1` ~ `::h6` 就是用它的行内包装写出来的，没有专门的标题展开器。
//!
//! 注册默认展开器用 [`register_defaults`]。想换行内配置，就自己构造一个
//! [`NaturalExpander`] 再注册。

mod natural;
mod wrap;

pub use natural::NaturalExpander;
pub use wrap::Wrap;

use crate::dispatch::{Matched, Registry};

/// 内置的标题模式：`h1` ~ `h6`。
///
/// 正则在这里能把级别写准，所以不需要别的校验——`ha`、`h7` 根本不会命中它，
/// 会落到兜底展开器上得到一个「未注册的调用块」报错。
pub const HEADING_PATTERN: &str = "^h[1-6]$";

/// 注册内置展开器。
///
/// * 自然块槽位：默认配置的 [`NaturalExpander`]；
/// * `::h1` ~ `::h6`：一条正则模式 [`HEADING_PATTERN`]，交给一个 [`Wrap`]。
///
/// 标题**没有专门的展开器**——它就是个包装器：标签名取正则匹配到的那一段
/// （`h1`~`h6`），类名加 `nm-` 前缀，正文按行内解析（`<h3>` 里不能有 `<p>`）。
pub fn register_defaults(registry: &mut Registry) {
    let natural = NaturalExpander::default();

    registry.register_natural(natural.clone());
    registry.register_pattern(
        regex::Regex::new(HEADING_PATTERN).expect("内置标题模式在测试里被验证过"),
        Wrap::tag_from(|matched: &Matched| matched.matched().to_string())
            .class_from(|matched: &Matched| format!("nm-{}", matched.matched()))
            .inline(natural),
    );
}
