//! HTML 渲染器：把**语义**节点映射成 HTML。
//!
//! AST 说「这是强调」，这里决定用 `<em class="nm-em">`；AST 说「这是一个
//! `rocket` 短码」，这里决定查 [`emoji`] 表并包一层 `<span>`。所以换标签、
//! 换类名、把 emoji 换成手搓 SVG，都只动这个模块。
//!
//! 唯一会绕过这张映射表的是 [`NodeKind::Element`]——调用块展开器（尤其是
//! 外部的）自己写好的 HTML 元素，这里按原样输出。
//!
//! 输出规则：
//!
//! * 只写 DOM。转义在这里做，AST 里存的始终是**未转义**文本。
//! * 顶层块之间插入换行以便阅读；**元素内部一个空白都不插**，否则
//!   `pre` 的内容会被破坏。
//! * [`NodeKind::Error`] 渲染成信息提示框。
//! * [`NodeKind::Unparsed`] 本不该出现（说明没跑调度器），画成提示框而不是
//!   静默丢内容。

use crate::ast::{Ast, Attr, Block, ErrorNode, NodeId, NodeKind, Params};

use super::icon;

/// 把整棵树渲染成 HTML 片段（**不含** `<html>` / `<body>` 外壳）。
pub fn render(ast: &Ast) -> String {
    let mut out = String::new();
    let roots: Vec<NodeId> = ast.children(ast.document()).collect();

    for (index, id) in roots.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        write_node(ast, *id, false, &mut out);
    }

    out
}

/// 渲染一个节点。
///
/// `inline` 表示**当前是否处在行内上下文**里（`<p>`、`<h1>`、`<a>` 里面）。
/// 它由渲染器**自己写下的标签**决定——写 `<p>` 的那个调用点就知道孩子们是
/// 行内的——所以不需要去猜父节点。
///
/// 它决定**同一个语义节点映射成哪个标签**：模板实例在块级位置是 `<div>`，
/// 在行内位置必须是 `<span>`，否则 `<p>` 里会塞进 `<div>`（非法 HTML）。
fn write_node(ast: &Ast, id: NodeId, inline: bool, out: &mut String) {
    match ast.kind(id) {
        Some(NodeKind::Document) => {
            for child in ast.children(id).collect::<Vec<_>>() {
                write_node(ast, child, false, out);
            }
        }

        // ── 块级 ──
        // 段落与标题的**内容**是行内的。
        Some(NodeKind::Paragraph) => container(ast, id, "p", "nm-p", true, out),
        Some(NodeKind::Heading { level }) => container(
            ast,
            id,
            &format!("h{level}"),
            &format!("nm-h{level}"),
            true,
            out,
        ),
        Some(NodeKind::Instance { name, params }) => {
            write_instance(ast, id, name, params, inline, out)
        }

        // ── 行内 ──
        Some(NodeKind::Emphasis) => container(ast, id, "em", "nm-em", true, out),
        Some(NodeKind::Strong) => container(ast, id, "strong", "nm-strong", true, out),
        Some(NodeKind::Strikethrough) => container(ast, id, "del", "nm-del", true, out),
        Some(NodeKind::Subscript) => container(ast, id, "sub", "nm-sub", true, out),
        Some(NodeKind::Superscript) => container(ast, id, "sup", "nm-sup", true, out),
        Some(NodeKind::Mark) => container(ast, id, "mark", "nm-mark", true, out),
        Some(NodeKind::Code) => container(ast, id, "code", "nm-code-inline", true, out),
        Some(NodeKind::Math) => write_math(ast, id, out),
        Some(NodeKind::Icon(name)) => write_icon(name, out),
        Some(NodeKind::InlineCall { name, params, .. }) => {
            write_inline_call(ast, id, name, params, out)
        }
        Some(NodeKind::LineBreak) => out.push_str("<br>"),
        Some(NodeKind::Link { target }) => write_link(ast, id, target, out),

        // ── 逃生口 ──
        Some(NodeKind::Element { tag, attrs }) => write_element(ast, id, tag, attrs, inline, out),

        // ── 内容 ──
        Some(NodeKind::Text(text)) => escape_text(text, out),
        // 报错自己可能已经知道落在行内；落在行内上下文里的一律按行内画。
        Some(NodeKind::Error(error)) => write_error(error, inline, out),
        // 未解析的块本不该出现在这里——说明渲染之前没有跑调度器。
        // 宁可显眼地画出来，也不要静默丢内容。
        Some(NodeKind::Unparsed(block)) => write_unparsed(block, inline, out),
        None => {}
    }
}

/// 一个带 `nm-` 类名的普通元素。
fn container(ast: &Ast, id: NodeId, tag: &str, class: &str, inner_inline: bool, out: &mut String) {
    out.push('<');
    out.push_str(tag);
    out.push_str(" class=\"");
    out.push_str(class);
    out.push_str("\">");

    for child in ast.children(id).collect::<Vec<_>>() {
        write_node(ast, child, inner_inline, out);
    }

    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

/// 逃生口：把 AST 里已经写好的 HTML 元素原样输出。
///
/// 标签名先做合法性校验——它不是用户输入，但展开器写错了也会产出坏 HTML。
/// 校验不过时**只输出孩子**，这样至少不丢内容。
fn write_element(ast: &Ast, id: NodeId, tag: &str, attrs: &[Attr], inline: bool, out: &mut String) {
    if !is_valid_attr_name(tag) {
        for child in ast.children(id).collect::<Vec<_>>() {
            write_node(ast, child, inline, out);
        }
        return;
    }

    out.push('<');
    out.push_str(tag);
    write_attrs(attrs, out);
    out.push('>');

    if is_void(tag) {
        return;
    }

    for child in ast.children(id).collect::<Vec<_>>() {
        write_node(ast, child, inline, out);
    }
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

fn write_attrs(attrs: &[Attr], out: &mut String) {
    for attr in attrs {
        if !is_valid_attr_name(&attr.name) {
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

/// 行内链接。
///
/// 目标转义后写进 `href`。**目前不校验 scheme**——`javascript:` 这类目标
/// 会照样写出去；要挡就在这里加一层白名单。
fn write_link(ast: &Ast, id: NodeId, target: &str, out: &mut String) {
    out.push_str("<a class=\"nm-link\" href=\"");
    escape_attr(target, out);
    out.push_str("\">");

    for child in ast.children(id).collect::<Vec<_>>() {
        write_node(ast, child, true, out);
    }

    out.push_str("</a>");
}

/// 行内数学。
///
/// AST 里存的是原样内容；这里归一成 KaTeX / MathJax 默认认识的 `\(...\)`，
/// 这样 `$$` 包裹内层 `$` 的写法不会泄漏成行间公式。要换分隔符改这里。
fn write_math(ast: &Ast, id: NodeId, out: &mut String) {
    out.push_str("<span class=\"nm-math\">\\(");
    for child in ast.children(id).collect::<Vec<_>>() {
        write_node(ast, child, true, out);
    }
    out.push_str("\\)</span>");
}

/// 行内图标：查 [`icon`] 表决定显示什么，认不出就把 `:name:` 原样回显。
///
/// 图标**不经过调度器**——它是呈现，不是逻辑。想让它变成别的样子就改表。
fn write_icon(name: &str, out: &mut String) {
    match icon::value(name) {
        Some(value) => {
            out.push_str("<span class=\"nm-icon\" data-alias=\"");
            escape_attr(name, out);
            out.push_str("\">");
            escape_text(value, out);
            out.push_str("</span>");
        }
        None => {
            out.push(':');
            escape_text(name, out);
            out.push(':');
        }
    }
}

/// 行内调用：按**规范形式**原样回显成 `{{name k=v: 内容}}`。
///
/// 正常情况下渲染器**看不到**这个节点：行内调用要么被展开器认领，要么由调度器
/// 交给兜底展开器变成报错节点。只有把**未经调度**的 AST 直接交给
/// [`render`] 时才会走到这里，所以这里取「不吞信息」的稳妥做法。
fn write_inline_call(ast: &Ast, id: NodeId, name: &str, params: &Params, out: &mut String) {
    let children: Vec<NodeId> = ast.children(id).collect();

    // 规范回显由**解析层**提供：引号规则与 `parse_call_header` 同一套，
    // 所以这里写出来的东西能原样再解析回去。
    let header = crate::parse::format_call_header(name, params);
    out.push_str("{{");
    escape_text(&header, out);

    if !children.is_empty() {
        out.push_str(": ");
        for child in children {
            write_node(ast, child, true, out);
        }
    }

    out.push_str("}}");
}

/// 模板实例的**默认**映射。
///
/// 目前还没有任何调用块展开器，所以先给一个通用形态：带类名的 `div`，
/// 参数以 `data-*` 带出来，方便 CSS/JS 取用。将来 `::notice` 之类的映射
/// 落地时，在这里按 `name` 分派即可。
///
/// 参数键来自调用头部（**用户输入**），所以先做名字合法性校验；`data-`
/// 前缀也顺带把 `onclick` 这类名字中和成无害的属性。
/// 模板实例：**按上下文选标签**。块级位置是 `<div>`，行内位置是 `<span>`——
/// 行内调用展开出的实例落在 `<p>` 里面，用 `<div>` 就是非法 HTML。
fn write_instance(
    ast: &Ast,
    id: NodeId,
    name: &str,
    params: &Params,
    inline: bool,
    out: &mut String,
) {
    let tag = if inline { "span" } else { "div" };

    out.push('<');
    out.push_str(tag);
    out.push_str(" class=\"nm-instance nm-instance-");
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
        write_node(ast, child, inline, out);
    }
    out.push_str("</");
    out.push_str(tag);
    out.push('>');
}

fn write_error(error: &ErrorNode, inline: bool, out: &mut String) {
    // 报错自己知道落在行内；此外，落在行内上下文里的报错一律按行内画，
    // 否则就会在 <p> 里塞一个 <div>。
    if error.inline || inline {
        return write_inline_error(error, out);
    }

    write_error_box(
        error.kind.as_str(),
        &error.message,
        Some(&error.content),
        out,
    );
}

/// 行内位置的报错：**不能**用 `<div>` 与 `<pre>`——它会落在 `<p>` 里面，
/// 那样是非法 HTML。所以只输出一行 `<span>`，原文靠 `title` 兜着。
fn write_inline_error(error: &ErrorNode, out: &mut String) {
    out.push_str("<span class=\"nm-error-inline nm-error-");
    out.push_str(error.kind.as_str());
    out.push_str("\" data-kind=\"");
    escape_attr(error.kind.as_str(), out);
    out.push_str("\" title=\"");
    escape_attr(&error.message, out);
    out.push_str("\">");

    // 显示**出错的那段调用原文**（规范形式）；没有承载文本时才退回说明。
    // 于是正文读起来是「看 {{quote}} 这一句还在。」——错误被标出来，句子还在。
    if error.content.is_empty() {
        escape_text(&error.message, out);
    } else {
        escape_text(&error.content, out);
    }

    out.push_str("</span>");
}

fn write_unparsed(block: &Block, inline: bool, out: &mut String) {
    let message = match block {
        Block::Call(call) => format!("未解析的调用块 ::{}", call.name),
        Block::Natural(_) => "未解析的自然块".to_string(),
    };

    if inline {
        // 同上：行内上下文里不能出现块级提示框。
        out.push_str(
            "<span class=\"nm-error-inline nm-error-unparsed\" data-kind=\"unparsed\" title=\"",
        );
        escape_attr(&message, out);
        out.push_str("\">");
        escape_text(&message, out);
        out.push_str("</span>");
        return;
    }

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
    fn icon_names_are_looked_up_in_the_table_here() {
        let mut ast = Ast::new();
        let id = ast.new_icon("rocket");
        ast.push_block(id);

        assert_eq!(
            render(&ast),
            "<span class=\"nm-icon\" data-alias=\"rocket\">🚀</span>"
        );
    }

    #[test]
    fn unknown_icon_names_are_echoed_verbatim() {
        let mut ast = Ast::new();
        let id = ast.new_icon("nope");
        ast.push_block(id);

        assert_eq!(render(&ast), ":nope:");
    }

    #[test]
    fn an_undispatched_inline_call_is_echoed_in_canonical_form() {
        // 正常流程里渲染器看不到行内调用（展开器认领或兜底报错）；只有把未经
        // 调度的 AST 直接渲染时才会走到这里，此时取「不吞信息」的做法。
        let mut ast = Ast::new();
        let id = ast.new_inline_call("nope", Params::new(), Span::new(1, 1, 0, 0));
        ast.push_block(id);

        assert_eq!(render(&ast), "{{nope}}");
    }

    #[test]
    fn echoed_headers_reparse_to_the_same_params() {
        let original: Params = [
            ("k 1".to_string(), "v 1".to_string()),
            ("k:2".to_string(), "v:2".to_string()),
            ("k=3".to_string(), "v 4".to_string()),
            ("plain".to_string(), "true".to_string()),
        ]
        .into_iter()
        .collect();

        let mut ast = Ast::new();
        let id = ast.new_inline_call("my name", original.clone(), Span::new(1, 1, 0, 0));
        ast.push_block(id);

        let echoed = render(&ast);
        let inner = echoed
            .strip_prefix("{{")
            .and_then(|rest| rest.strip_suffix("}}"))
            .expect("无展开器的行内调用回显成 {{…}}");

        let header = crate::parse::parse_call_header(inner);
        assert_eq!(header.name, "my name");
        assert_eq!(header.params, original);
    }

    #[test]
    fn unknown_inline_calls_with_params_or_content_echo_their_braced_form() {
        let params: Params = [("author".to_string(), "张三".to_string())]
            .into_iter()
            .collect();

        let mut ast = Ast::new();
        let id = ast.new_inline_call("quote", params, Span::new(1, 1, 0, 0));
        let text = ast.new_text("引用");
        ast.append(id, text);
        ast.push_block(id);

        assert_eq!(render(&ast), "{{quote author=张三: 引用}}");
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

    #[test]
    fn raw_elements_are_emitted_verbatim() {
        let mut ast = Ast::new();
        let pre = ast.new_element("pre", vec![Attr::new("class", "code")]);
        let code = ast.new_element("code", Vec::new());
        let text = ast.new_text("fn main() {}");
        ast.append(code, text);
        ast.append(pre, code);
        ast.push_block(pre);

        assert_eq!(
            render(&ast),
            "<pre class=\"code\"><code>fn main() {}</code></pre>"
        );
    }

    #[test]
    fn void_raw_elements_get_no_closing_tag() {
        let mut ast = Ast::new();
        let id = ast.new_element(
            "img",
            vec![Attr::new("src", "a.png"), Attr::boolean("lazy")],
        );
        ast.push_block(id);

        assert_eq!(render(&ast), "<img src=\"a.png\" lazy>");
    }

    #[test]
    fn raw_element_attributes_are_escaped_and_invalid_names_dropped() {
        let mut ast = Ast::new();
        let id = ast.new_element(
            "div",
            vec![Attr::new("title", "a\"b&c"), Attr::new("bad name", "x")],
        );
        ast.push_block(id);

        assert_eq!(render(&ast), "<div title=\"a&quot;b&amp;c\"></div>");
    }

    #[test]
    fn a_raw_element_with_an_invalid_tag_keeps_only_its_children() {
        // 标签名写坏了不该把内容一起丢掉。
        let mut ast = Ast::new();
        let id = ast.new_element("bad tag", Vec::new());
        let text = ast.new_text("内容还在");
        ast.append(id, text);
        ast.push_block(id);

        assert_eq!(render(&ast), "内容还在");
    }
}
