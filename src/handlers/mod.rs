//! 展开器的**机制**。
//!
//! 这里放的是「怎么写展开器」，具体的展开器在 [`stdlib`](crate::stdlib)。分开的
//! 理由：机制会随内核演进（要改就改这里），标准库只会越来越长（加就加那边）。
//!
//! * [`NaturalExpander`]——自然块 → 段落（或行内内容）。它既是一个
//!   [`Handler`](crate::dispatch::Handler)，也是一个**可直接调用的接口**，
//!   其他展开器解析文本时用它。
//! * [`Wrap`]——把调用套一层标签/带 class 的 div。**大多数展开器就干这件事**，
//!   所以它是注册时的语法糖；读参数、读捕获组、选块级还是行内包装都在这里。
//!
//! 想一次装上内置展开器用 [`register_defaults`](crate::stdlib::register_defaults)。

mod natural;
mod wrap;

pub use natural::NaturalExpander;
pub use wrap::Wrap;
