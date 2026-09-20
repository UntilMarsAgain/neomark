//! 引用块：`::quote origin=出处`。

use crate::dispatch::Registry;
use crate::handlers::Wrap;

/// 调用名。
pub const NAME: &str = "quote";

/// 注册引用块展开器。
///
/// ```text
/// ::quote origin="《哥德尔、艾舍尔、巴赫》"
///   正文照常按块级解析，里面可以有段落、列表、别的调用块。
/// ```
///
/// 产出 `<blockquote class="nm-quote" data-origin="…">`。没有 `origin` 就不写这个
/// 属性——参数缺席与参数为空是两件事，缺席时不该留下一个空属性。
///
/// 用 `data-origin` 而不是 HTML 的 `cite`：按规范 `cite` 的值是个 **URL**，而作者
/// 写进来的多半是书名、讲者、演讲名这类自由文本，套 `cite` 是假语义。
pub fn register(registry: &mut Registry) {
    registry.register(
        NAME,
        Wrap::tag("blockquote")
            .class("nm-quote")
            .attr_from_param("origin", "data-origin"),
    );
}
