//! 代码块：`::code language=rust`。

use crate::ast::{Ast, Attr, NodeId};
use crate::dispatch::{Context, Handler, Matched, Registry};

/// 调用名。
pub const NAME: &str = "code";

/// 注册代码块展开器。
///
/// ```text
/// ::code language=rust: fn main() {
///   println!("首行内容 + 缩进的续行");
/// }
/// ```
///
/// 产出 `<pre class="nm-code-block"><code class="language-rust">…</code></pre>`。
/// `language-xxx` 是 **highlight.js / Prism 认的写法**，把它交给页面上的高亮脚本
/// 即可——**本库不做高亮，也不引入任何 JS**。
///
/// 块体**原样**输出：不解析行内标记、不碰缩进，只由渲染器做必要的转义。这也正是
/// 它不能用[包装器](crate::handlers::Wrap)写的原因——包装器会把块体当块级结构继续
/// 展开，而代码要的是原文。
pub fn register(registry: &mut Registry) {
    registry.register(NAME, Code);
}

struct Code;

impl Handler for Code {
    fn expand_call(
        &self,
        node: NodeId,
        ast: &mut Ast,
        _ctx: &mut Context<'_>,
        _matched: &Matched<'_>,
    ) -> Vec<NodeId> {
        // 先读出语言与原文（这一段要借 `Ast`），再动树。
        let (language, source) = {
            let Some(call) = ast.call(node) else {
                return Vec::new();
            };

            (
                call.params.get("language").map(language_class),
                call.raw_body.clone(),
            )
        };

        let pre = ast.new_element("pre", vec![Attr::new("class", "nm-code-block")]);
        let code = ast.new_element(
            "code",
            language.map_or_else(Vec::new, |class| vec![Attr::new("class", class)]),
        );
        let text = ast.new_text(source);

        ast.append(code, text);
        ast.append(pre, code);

        vec![pre]
    }
}

/// `rust` → `language-rust`。
///
/// 语言名里 `+` `#` `.` `-` 都是合法的（`c++`、`c#`、`objective-c`、`f#`），留着；
/// 其余字符换成 `-`，免得把类名弄出空格、或串进别的类名里去。
fn language_class(language: &str) -> String {
    let mut class = String::from("language-");
    class.extend(language.chars().map(|c| {
        if c.is_ascii_alphanumeric() || matches!(c, '+' | '#' | '.' | '-' | '_') {
            c
        } else {
            '-'
        }
    }));
    class
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_names_keep_the_characters_highlighters_use() {
        assert_eq!(language_class("rust"), "language-rust");
        assert_eq!(language_class("c++"), "language-c++");
        assert_eq!(language_class("c#"), "language-c#");
        assert_eq!(language_class("objective-c"), "language-objective-c");
    }

    #[test]
    fn a_dirty_language_name_cannot_break_out_of_the_class_attribute() {
        assert_eq!(
            language_class("ru st\"><script>"),
            "language-ru-st---script-"
        );
    }
}
