//! HTML 渲染：把已展开的块树写成 HTML。
//!
//! * [`render`] 只输出 DOM 片段；
//! * [`render_page`] 输出自包含的完整页面，内嵌 [`DEFAULT_CSS`]。
//!
//! 这一层只依赖 [`crate::ast`]，单向读取，不改树。

mod document;
mod render;

pub use document::{DEFAULT_CSS, render_page};
pub use render::render;
