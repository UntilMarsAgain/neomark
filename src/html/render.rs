//! HTML 渲染器：遍历块树，写出 HTML 片段。

use crate::ast::{Ast, Attr, Block, ErrorNode, NodeId, NodeKind};

/// 把整棵树渲染成 HTML 片段（**不含** `<html>` / `<body>` 外壳）。
///
/// * 只写 DOM。转义在这里做，块树里存的始终是**未转义**文本。
/// * 顶层块之间插入换行以便阅读；**元素内部一个空白都不插**，否则
///   `pre` 的内容会被破坏。
/// * `Error` 节点渲染成信息提示框。
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
        Some(NodeKind::Element { tag, attrs }) => write_element(ast, id, tag, attrs, out),
        Some(NodeKind::Text(text)) => escape_text(text, out),
        Some(NodeKind::Error(error)) => write_error(error, out),
        // 未解析的块本不该出现在这里——说明渲染之前没有跑调度器。
        // 宁可显眼地画出来，也不要静默丢内容。
        Some(NodeKind::Unparsed(block)) => write_unparsed(block, out),
        None => {}
    }
}

fn write_element(ast: &Ast, id: NodeId, tag: &str, attrs: &[Attr], out: &mut String) {
    out.push('<');
    out.push_str(tag);
    write_attrs(attrs, out);
    out.push('>');

    if is_void(tag) {
        return;
    }

    for child in ast.children(id).collect::<Vec<_>>() {
        write_node(ast, child, out);
    }

    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

fn write_attrs(attrs: &[Attr], out: &mut String) {
    for attr in attrs {
        if !is_valid_attr_name(&attr.name) {
            // 属性名目前来自展开器而非用户输入，但一旦引入「参数 → 属性」的
            // 通用映射，它就会变成注入面，所以这里先从语法上挡掉。
            continue;
        }
        out.push(' ');
        out.push_str(&attr.name);
        if let Some(value) = &attr.value {
            out.push_str("=\"");
            escape_attr(value, out);
            out.push('"');
        }
    }
}

/// 属性名只允许 `[A-Za-z0-9_:.-]`。
fn is_valid_attr_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':' | b'.'))
}

/// HTML5 的 void 元素：没有闭合标签。
const VOID_ELEMENTS: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

fn is_void(tag: &str) -> bool {
    VOID_ELEMENTS
        .iter()
        .any(|void| void.eq_ignore_ascii_case(tag))
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
        Block::Call(call) => format!("未展开的调用块 ::{}", call.name),
        Block::Natural(_) => "未展开的自然块".to_string(),
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
    use crate::ast::{Attr, ErrorKind, ErrorNode, Span};
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
        // 原文原样回显，并且是转义过的。
        assert!(
            html.contains("<pre class=\"nm-error-content\"><code>::nope a=1:\n  body</code></pre>")
        );
    }

    #[test]
    fn void_elements_have_no_closing_tag() {
        let mut ast = Ast::new();
        let image = ast.new_element(
            "img",
            vec![Attr::new("src", "a.png"), Attr::boolean("lazy")],
        );
        ast.push_block(image);

        assert_eq!(render(&ast), "<img src=\"a.png\" lazy>");
    }

    #[test]
    fn attribute_values_are_escaped() {
        let mut ast = Ast::new();
        let div = ast.new_element("div", vec![Attr::new("title", "a\"b&c")]);
        ast.push_block(div);

        assert_eq!(render(&ast), "<div title=\"a&quot;b&amp;c\"></div>");
    }

    #[test]
    fn invalid_attribute_names_are_dropped() {
        let mut ast = Ast::new();
        let div = ast.new_element(
            "div",
            vec![
                Attr::new("class", "ok"),
                Attr::new("bad name", "x"),
                Attr::new("bad\"name", "y"),
            ],
        );
        ast.push_block(div);

        assert_eq!(render(&ast), "<div class=\"ok\"></div>");
    }

    #[test]
    fn an_unparsed_node_is_drawn_rather_than_dropped() {
        // 忘了跑调度器时不该静默丢内容。
        let ast = parse("::code:\n  x");
        let html = render(&ast);

        assert!(html.contains("nm-error-unparsed"));
        assert!(html.contains("未展开的调用块 ::code"));
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
}
