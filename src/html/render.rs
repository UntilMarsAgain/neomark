//! HTML 渲染器：把**语义**节点映射成 HTML。
//!
//! 这是整个 crate 里唯一知道 HTML 的地方。AST 说「这是强调」，这里决定
//! 用 `<em class="nm-em">`；AST 说「这是一个 `rocket` 短码」，这里决定查
//! [`emoji`] 表并包一层 `<span>`。所以换标签、换类名、把 emoji 换成手搓
//! SVG，都只动这个模块。
//!
//! 输出规则：
//!
//! * 只写 DOM。转义在这里做，AST 里存的始终是**未转义**文本。
//! * 顶层块之间插入换行以便阅读；**元素内部一个空白都不插**，否则
//!   `pre` 的内容会被破坏。
//! * [`NodeKind::Error`] 渲染成信息提示框。
//! * [`NodeKind::Unparsed`] 本不该出现（说明没跑调度器），画成提示框而不是
//!   静默丢内容。

use crate::ast::{Ast, Block, ErrorNode, NodeId, NodeKind, Params};

use super::emoji;

/// 把整棵树渲染成 HTML 片段（**不含** `<html>` / `<body>` 外壳）。
pub fn render(ast: &Ast) -> String {
    let mut out = String::new();
    let roots: Vec<NodeId> = ast.children(ast.document()).collect();

    for (index, id) in roots.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        write_node(ast, *id, &mut out);
    }

    out
}

fn write_node(ast: &Ast, id: NodeId, out: &mut String) {
    match ast.kind(id) {
        Some(NodeKind::Document) => {
            for child in ast.children(id).collect::<Vec<_>>() {
                write_node(ast, child, out);
            }
        }

        // ── 块级 ──
        Some(NodeKind::Paragraph) => container(ast, id, "p", "nm-p", out),
        Some(NodeKind::Instance { name, params }) => write_instance(ast, id, name, params, out),

        // ── 行内 ──
        Some(NodeKind::Emphasis) => container(ast, id, "em", "nm-em", out),
        Some(NodeKind::Strong) => container(ast, id, "strong", "nm-strong", out),
        Some(NodeKind::Strikethrough) => container(ast, id, "del", "nm-del", out),
        Some(NodeKind::Subscript) => container(ast, id, "sub", "nm-sub", out),
        Some(NodeKind::Superscript) => container(ast, id, "sup", "nm-sup", out),
        Some(NodeKind::Mark) => container(ast, id, "mark", "nm-mark", out),
        Some(NodeKind::Code) => container(ast, id, "code", "nm-code-inline", out),
        Some(NodeKind::Math) => write_math(ast, id, out),
        Some(NodeKind::Emoji(alias)) => write_emoji(alias, out),
        Some(NodeKind::LineBreak) => out.push_str("<br>"),

        // ── 内容 ──
        Some(NodeKind::Text(text)) => escape_text(text, out),
        Some(NodeKind::Error(error)) => write_error(error, out),
        // 未解析的块本不该出现在这里——说明渲染之前没有跑调度器。
        // 宁可显眼地画出来，也不要静默丢内容。
        Some(NodeKind::Unparsed(block)) => write_unparsed(block, out),
        None => {}
    }
}

/// 一个带 `nm-` 类名的普通元素。
fn container(ast: &Ast, id: NodeId, tag: &str, class: &str, out: &mut String) {
    out.push('<');
    out.push_str(tag);
    out.push_str(" class=\"");
    out.push_str(class);
    out.push_str("\">");

    for child in ast.children(id).collect::<Vec<_>>() {
        write_node(ast, child, out);
    }

    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

/// 行内数学。
///
/// AST 里存的是原样内容；这里归一成 KaTeX / MathJax 默认认识的 `\(...\)`，
/// 这样 `$$` 包裹内层 `$` 的写法不会泄漏成行间公式。要换分隔符改这里。
fn write_math(ast: &Ast, id: NodeId, out: &mut String) {
    out.push_str("<span class=\"nm-math\">\\(");
    for child in ast.children(id).collect::<Vec<_>>() {
        write_node(ast, child, out);
    }
    out.push_str("\\)</span>");
}

/// Emoji 短码：查表决定长什么样。
///
/// 表里没有的名字**原样回显**，不吞掉作者的输入。想换成手搓 SVG 或加上
/// `title`，改 [`emoji`] 表与这个函数即可。
fn write_emoji(alias: &str, out: &mut String) {
    match emoji::value(alias) {
        Some(value) => {
            out.push_str("<span class=\"nm-emoji\" data-alias=\"");
            escape_attr(alias, out);
            out.push_str("\">");
            escape_text(value, out);
            out.push_str("</span>");
        }
        None => {
            out.push(':');
            escape_text(alias, out);
            out.push(':');
        }
    }
}

/// 模板实例的**默认**映射。
///
/// 目前还没有任何调用块展开器，所以先给一个通用形态：带类名的 `div`，
/// 参数以 `data-*` 带出来，方便 CSS/JS 取用。将来 `::notice` 之类的映射
/// 落地时，在这里按 `name` 分派即可。
///
/// 参数键来自调用头部（**用户输入**），所以先做名字合法性校验；`data-`
/// 前缀也顺带把 `onclick` 这类名字中和成无害的属性。
fn write_instance(ast: &Ast, id: NodeId, name: &str, params: &Params, out: &mut String) {
    out.push_str("<div class=\"nm-instance nm-instance-");
    escape_attr(name, out);
    out.push_str("\" data-name=\"");
    escape_attr(name, out);
    out.push('"');

    for (key, value) in params.iter() {
        if !is_valid_attr_name(key) {
            continue;
        }
        out.push_str(" data-");
        out.push_str(key);
        out.push_str("=\"");
        escape_attr(value, out);
        out.push('"');
    }

    out.push('>');
    for child in ast.children(id).collect::<Vec<_>>() {
        write_node(ast, child, out);
    }
    out.push_str("</div>");
}

fn write_error(error: &ErrorNode, out: &mut String) {
    write_error_box(
        error.kind.as_str(),
        &error.message,
        Some(&error.content),
        out,
    );
}

fn write_unparsed(block: &Block, out: &mut String) {
    let message = match block {
        Block::Call(call) => format!("未解析的调用块 ::{}", call.name),
        Block::Natural(_) => "未解析的自然块".to_string(),
    };
    write_error_box("unparsed", &message, None, out);
}

/// 信息提示框：外层 + 一行说明 +（可选）原样回显的块原文。
fn write_error_box(modifier: &str, message: &str, content: Option<&str>, out: &mut String) {
    out.push_str("<div class=\"nm-error nm-error-");
    out.push_str(modifier);
    out.push_str("\" data-kind=\"");
    escape_attr(modifier, out);
    out.push_str("\"><p class=\"nm-error-message\">");
    escape_text(message, out);
    out.push_str("</p>");

    if let Some(content) = content {
        out.push_str("<pre class=\"nm-error-content\"><code>");
        escape_text(content, out);
        out.push_str("</code></pre>");
    }

    out.push_str("</div>");
}

/// 属性名只允许 `[A-Za-z0-9_:.-]`。
///
/// 参数键来自调用头部，是用户输入，不能直接拼进标签。
fn is_valid_attr_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':' | b'.'))
}

/// 转义正文：`&` `<` `>`。
pub(crate) fn escape_text(text: &str, out: &mut String) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
}

/// 转义属性值：在正文的基础上再加 `"` 与 `'`。
fn escape_attr(text: &str, out: &mut String) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ErrorKind, Params, Span};
    use crate::dispatch::{Context, Dispatcher, Registry};
    use crate::handlers;
    use crate::parse::parse;

    fn render_source(source: &str) -> String {
        let mut ast = parse(source);
        let mut registry = Registry::new();
        handlers::register_defaults(&mut registry);

        let mut ctx = Context::new(source);
        Dispatcher::new(registry).run(&mut ast, &mut ctx);

        render(&ast)
    }

    #[test]
    fn paragraphs_are_wrapped_and_escaped() {
        assert_eq!(
            render_source("a < b & c > d"),
            "<p class=\"nm-p\">a &lt; b &amp; c &gt; d</p>"
        );
    }

    #[test]
    fn top_level_blocks_are_separated_by_newlines() {
        assert_eq!(
            render_source("第一段\n\n第二段"),
            "<p class=\"nm-p\">第一段</p>\n<p class=\"nm-p\">第二段</p>"
        );
    }

    #[test]
    fn error_nodes_become_a_message_box_with_their_content() {
        let html = render_source("::nope a=1:\n  body");

        assert!(html.starts_with("<div class=\"nm-error nm-error-no-handler\""));
        assert!(html.contains("<p class=\"nm-error-message\">未注册的调用块 ::nope</p>"));
        assert!(
            html.contains("<pre class=\"nm-error-content\"><code>::nope a=1:\n  body</code></pre>")
        );
    }

    #[test]
    fn error_kinds_map_to_their_own_class_modifier() {
        let mut ast = Ast::new();
        let error = ast.new_error(ErrorNode::new(
            ErrorKind::ExpandFailed,
            "炸了",
            Span::new(1, 1, 0, 0),
            "原文",
        ));
        ast.push_block(error);

        let html = render(&ast);
        assert!(html.starts_with("<div class=\"nm-error nm-error-expand-failed\""));
        assert!(html.contains("<p class=\"nm-error-message\">炸了</p>"));
    }

    #[test]
    fn an_unparsed_node_is_drawn_rather_than_dropped() {
        // 忘了跑调度器时不该静默丢内容。
        let ast = parse("::code:\n  x");
        let html = render(&ast);

        assert!(html.contains("nm-error-unparsed"));
        assert!(html.contains("未解析的调用块 ::code"));
    }

    #[test]
    fn emoji_aliases_are_looked_up_here() {
        let mut ast = Ast::new();
        let id = ast.new_emoji("rocket");
        ast.push_block(id);

        assert_eq!(
            render(&ast),
            "<span class=\"nm-emoji\" data-alias=\"rocket\">🚀</span>"
        );
    }

    #[test]
    fn unknown_emoji_aliases_are_echoed_verbatim() {
        let mut ast = Ast::new();
        let id = ast.new_emoji("nope");
        ast.push_block(id);

        assert_eq!(render(&ast), ":nope:");
    }

    #[test]
    fn a_line_break_is_a_void_br() {
        let mut ast = Ast::new();
        let id = ast.new_line_break();
        ast.push_block(id);

        assert_eq!(render(&ast), "<br>");
    }

    #[test]
    fn instances_get_a_generic_mapping_with_params_as_data_attributes() {
        let params: Params = [("type".to_string(), "warning".to_string())]
            .into_iter()
            .collect();

        let mut ast = Ast::new();
        let instance = ast.new_instance("notice", params);
        let text = ast.new_text("小心");
        ast.append(instance, text);
        ast.push_block(instance);

        assert_eq!(
            render(&ast),
            "<div class=\"nm-instance nm-instance-notice\" data-name=\"notice\" data-type=\"warning\">小心</div>"
        );
    }

    #[test]
    fn param_keys_that_are_not_valid_attribute_names_are_dropped() {
        let params: Params = [
            ("type".to_string(), "warning".to_string()),
            ("bad name".to_string(), "x".to_string()),
            ("bad\"key".to_string(), "y".to_string()),
        ]
        .into_iter()
        .collect();

        let mut ast = Ast::new();
        let id = ast.new_instance("notice", params);
        ast.push_block(id);

        let html = render(&ast);
        assert!(html.contains(" data-type=\"warning\""));
        assert!(!html.contains("bad"));
    }
}
