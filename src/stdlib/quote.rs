//! 引用块：`::quote origin=出处`。

use crate::ast::{Ast, Attr, NodeId};
use crate::dispatch::{Context, Handler, Matched, Registry};

/// 调用名。
pub const NAME: &str = "quote";

/// 出处那一行的类名。
pub const ORIGIN_CLASS: &str = "nm-quote-origin";

/// 注册引用块展开器。
///
/// ```text
/// ::quote origin=马丁·路德·金
///   我有一个梦想！
/// ```
///
/// 产出：
///
/// ```html
/// <blockquote class="nm-quote" data-origin="马丁·路德·金">
///   <p class="nm-p">我有一个梦想！</p>
///   <footer class="nm-quote-origin">——马丁·路德·金</footer>
/// </blockquote>
/// ```
///
/// # 出处为什么是一行真的节点
///
/// 它**既进属性也进正文**：
///
/// * `data-origin` 给机器读。用 `data-*` 而不是 HTML 的 `cite`，因为按规范
///   `cite` 的值是个 **URL**，而作者写进来的多半是书名、讲者这类自由文本；
/// * `<footer>` 是给人读的那一行，右对齐、稍小号由 CSS 管。它是**真实文本**，
///   可以选中、复制、被读屏念出来——所以不用 CSS 的 `::after` 去生成，那样关掉
///   样式表出处就整个消失了。
///
/// 放在 `blockquote` **内部**用 `<footer>`，是 HTML 规范给引文出处的写法。
///
/// 没有 `origin` 就两样都不出现；参数缺席与参数为空都会当作「没有出处」。
/// 块体照常按块级解析，内层可以再有段落、列表、别的调用块。
pub fn register(registry: &mut Registry) {
    registry.register(NAME, Quote);
}

struct Quote;

impl Handler for Quote {
    fn expand_call(
        &self,
        node: NodeId,
        ast: &mut Ast,
        _ctx: &mut Context<'_>,
        _matched: &Matched<'_>,
    ) -> Vec<NodeId> {
        // 先读出出处（这一段要借 `Ast`），再动树。
        let origin = {
            let Some(call) = ast.call(node) else {
                return Vec::new();
            };

            call.params
                .get("origin")
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        };

        let mut attrs = vec![Attr::new("class", "nm-quote")];
        if let Some(origin) = &origin {
            attrs.push(Attr::new("data-origin", origin.clone()));
        }

        let blockquote = ast.new_element("blockquote", attrs);

        // 移交块体：一次 append 就把原有子节点搬过来了，内层照常继续展开。
        for child in ast.children(node).collect::<Vec<_>>() {
            ast.append(blockquote, child);
        }

        if let Some(origin) = origin {
            let footer = ast.new_element("footer", vec![Attr::new("class", ORIGIN_CLASS)]);
            let text = ast.new_text(format!("——{origin}"));

            ast.append(footer, text);
            ast.append(blockquote, footer);
        }

        vec![blockquote]
    }
}
