//! 端到端：源文本 → 解析 → 展开 → HTML。
//!
//! 这里只用公共 API，模拟外部使用者。

use neomark::{
    Ast, Context, Dispatcher, Handler, Matched, NodeId, Registry, handlers, html, parse,
};
use regex::Regex;

fn render(source: &str) -> String {
    let mut ast = parse(source);
    let mut registry = Registry::new();
    handlers::register_defaults(&mut registry);

    let mut ctx = Context::new(source);
    Dispatcher::new(registry).run(&mut ast, &mut ctx);

    html::render(&ast)
}

/// 在默认展开器之上再注册一些东西，然后渲染。
fn render_with(source: &str, extra: impl FnOnce(&mut Registry)) -> String {
    let mut ast = parse(source);
    let mut registry = Registry::new();
    handlers::register_defaults(&mut registry);
    extra(&mut registry);

    let mut ctx = Context::new(source);
    Dispatcher::new(registry).run(&mut ast, &mut ctx);
    html::render(&ast)
}

fn render_page(source: &str, title: &str) -> String {
    let mut ast = parse(source);
    let mut registry = Registry::new();
    handlers::register_defaults(&mut registry);

    let mut ctx = Context::new(source);
    Dispatcher::new(registry).run(&mut ast, &mut ctx);

    html::render_page(&ast, title)
}

/// 把调用名与参数原样写成一行文字，用来从 HTML 里观察解析结果。
struct EchoSignature;

impl Handler for EchoSignature {
    fn expand_call(
        &self,
        node: NodeId,
        ast: &mut Ast,
        _ctx: &mut Context<'_>,
        _matched: &Matched<'_>,
    ) -> Vec<NodeId> {
        let call = ast.call(node).unwrap();
        let mut text = format!("[{}", call.name);
        for (key, value) in call.params.iter() {
            text.push_str(&format!(" {key}={value}"));
        }
        text.push(']');

        let paragraph = ast.new_paragraph();
        let text = ast.new_text(text);
        ast.append(paragraph, text);

        vec![paragraph]
    }
}

/// 行内展开器：把行内调用展开成一个模板实例（最容易踩到行内/块级标签的地方）。
struct InlineBadge;

impl Handler for InlineBadge {
    fn expand_inline(
        &self,
        node: NodeId,
        ast: &mut Ast,
        _ctx: &mut Context<'_>,
        _matched: &Matched<'_>,
    ) -> Vec<NodeId> {
        let (name, params, _) = ast.inline_call(node).unwrap();
        let (name, params) = (name.to_string(), params.clone());

        let instance = ast.new_instance(name, params);
        for child in ast.children(node).collect::<Vec<_>>() {
            ast.append(instance, child);
        }
        vec![instance]
    }
}

/// 行内展开器：产出**块级**元素——最容易把 `<p>` 弄坏的做法。
struct InlineDiv;

impl Handler for InlineDiv {
    fn expand_inline(
        &self,
        node: NodeId,
        ast: &mut Ast,
        _ctx: &mut Context<'_>,
        _matched: &Matched<'_>,
    ) -> Vec<NodeId> {
        let element = ast.new_element("div", vec![neomark::Attr::new("class", "box")]);
        for child in ast.children(node).collect::<Vec<_>>() {
            ast.append(element, child);
        }
        vec![element]
    }
}

#[test]
fn a_plain_document_becomes_paragraphs() {
    assert_eq!(
        render("第一段\n\n第二段"),
        "<p class=\"nm-p\">第一段</p>\n<p class=\"nm-p\">第二段</p>"
    );
}

#[test]
fn markup_that_looks_like_html_is_escaped() {
    let html = render("if a < b && c > d { <script>alert(1)</script> }");

    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(html.contains("a &lt; b &amp;&amp; c &gt; d"));
}

#[test]
fn an_unknown_call_block_becomes_a_notice_box_without_stopping_the_rest() {
    let html = render("前面\n\n::notcie type=warning:\n  正文\n\n后面");

    // 前后两段照常渲染。
    assert!(html.contains("<p class=\"nm-p\">前面</p>"));
    assert!(html.contains("<p class=\"nm-p\">后面</p>"));

    // 中间那块变成一个提示框，且原文没丢——足够一眼看出块名拼错了。
    assert!(html.contains("nm-error-no-handler"));
    assert!(html.contains("未注册的调用块 ::notcie"));
    assert!(html.contains(
        "<pre class=\"nm-error-content\"><code>::notcie type=warning:\n  正文</code></pre>"
    ));
}

#[test]
fn a_call_block_swallows_its_subtree_into_one_box() {
    // 没有 ::code 展开器时，::code 整块（连同内部嵌套的块）变成一个提示框，
    // 内部不会再各自爆出提示框。
    let source = "::code lang=neomark:\n  ::notice:\n    内层\n";
    let html = render(source);

    assert_eq!(html.matches("nm-error-no-handler").count(), 1);
    assert!(html.contains("内层"));
}

#[test]
fn the_full_page_is_self_contained() {
    let page = render_page("你好", "示例");

    assert!(page.starts_with("<!DOCTYPE html>"));
    assert!(page.contains("<title>示例</title>"));
    assert!(page.contains("<style>"));
    assert!(page.contains(".nm-p {"));
    assert!(page.contains("<p class=\"nm-p\">你好</p>"));
    assert!(page.ends_with("</html>\n"));
    // 不依赖任何外部资源。
    assert!(!page.contains("<link "));
    assert!(!page.contains("<script"));
}

#[test]
fn crlf_input_compiles_the_same_as_lf() {
    assert_eq!(render("甲\r\n\r\n乙"), render("甲\n\n乙"));
}

#[test]
fn inline_markup_is_rendered_inside_paragraphs() {
    assert_eq!(
        render("**粗**、*斜*、~~删~~、`码`、==亮==、2^10^、~下~。"),
        concat!(
            "<p class=\"nm-p\">",
            "<strong class=\"nm-strong\">粗</strong>、",
            "<em class=\"nm-em\">斜</em>、",
            "<del class=\"nm-del\">删</del>、",
            "<code class=\"nm-code-inline\">码</code>、",
            "<mark class=\"nm-mark\">亮</mark>、",
            "2<sup class=\"nm-sup\">10</sup>、",
            "<sub class=\"nm-sub\">下</sub>。",
            "</p>"
        )
    );
}

#[test]
fn math_is_verbatim_so_its_markers_are_not_reinterpreted() {
    // `^` 在公式里不该被当成上标——数学的绑定比行内标记更紧。
    assert_eq!(
        render("$a^b$"),
        "<p class=\"nm-p\"><span class=\"nm-math\">\\(a^b\\)</span></p>"
    );
    // 代码跨度同理
    assert_eq!(
        render("`**a**`"),
        "<p class=\"nm-p\"><code class=\"nm-code-inline\">**a**</code></p>"
    );
}

#[test]
fn entities_icons_escapes_and_font_punctuation() {
    assert_eq!(
        render("&amp; &hellip; :rocket: \\*不斜\\* ..."),
        "<p class=\"nm-p\">&amp; … <span class=\"nm-icon\" data-alias=\"rocket\">🚀</span> *不斜* …</p>"
    );
}

#[test]
fn icon_lookup_happens_in_the_renderer_not_the_parser() {
    // 解析层只带走名字：渲染器表里有就显示图标，没有就原样回显。
    assert_eq!(
        render(":smile: 和 :nope:"),
        concat!(
            "<p class=\"nm-p\">",
            "<span class=\"nm-icon\" data-alias=\"smile\">😄</span> 和 :nope:",
            "</p>"
        )
    );
}

#[test]
fn the_colon_form_and_the_braced_form_are_two_different_things() {
    // `:smile:` 是**图标**：渲染器查表，显示成样子。
    assert_eq!(
        render(":smile:"),
        "<p class=\"nm-p\"><span class=\"nm-icon\" data-alias=\"smile\">😄</span></p>"
    );

    // `{{smile}}` 是**行内调用**：交给展开器，没人认领就是错误。
    let html = render("{{smile}}");
    assert!(html.contains("nm-error-inline"), "{html}");
    assert!(html.contains("没有展开器能处理行内调用 {smile}"), "{html}");
}

#[test]
fn an_unknown_inline_call_becomes_an_inline_error_not_a_block() {
    // 行内调用与块调用一样，没人认领就是错误；但报错落在**行内**位置，
    // 所以必须渲染成 <span>——否则会塞进 <p> 里，成为非法 HTML。
    let html = render("看 {{quote}} 这里");

    assert!(
        html.contains("<span class=\"nm-error-inline nm-error-no-handler\""),
        "{html}"
    );
    assert!(!html.contains("<div class=\"nm-error"), "{html}");
    // 可见文本是**出错的那段调用**，说明放在 title 里
    assert!(html.contains(">{{quote}}</span>"), "{html}");
    assert!(
        html.contains("title=\"没有展开器能处理行内调用 {quote}\""),
        "{html}"
    );
}

#[test]
fn a_braced_inline_call_does_not_swallow_the_rest_of_the_line() {
    // 这正是选 `{{ }}` 的理由：闭合符是独立记号，同一行后面的正文还在。
    let html = render("看 {{quote}} 这里");

    assert!(html.starts_with("<p class=\"nm-p\">看 "), "{html}");
    assert!(html.ends_with(" 这里</p>"), "{html}");
}

#[test]
fn quoting_survives_all_the_way_to_the_expander() {
    // 四处引号规则在**解析结果**上对不对，由展开器读到的东西说了算。
    let html = render_with("::echo \"k 1\"=\"v 1\" \"k:2\"=\"v:2\"", |registry| {
        registry.register("echo", EchoSignature);
    });

    assert_eq!(html, "<p class=\"nm-p\">[echo k 1=v 1 k:2=v:2]</p>");
}

#[test]
fn a_quoted_link_target_may_contain_spaces() {
    assert_eq!(
        render("[[文本 => \"a b\"]]"),
        "<p class=\"nm-p\"><a class=\"nm-link\" href=\"a b\">文本</a></p>"
    );
}

#[test]
fn a_quoted_link_target_may_contain_an_arrow() {
    // 引号保护目标里的箭头
    assert_eq!(
        render("[[a => \"b => c\"]]"),
        "<p class=\"nm-p\"><a class=\"nm-link\" href=\"b =&gt; c\">a</a></p>"
    );
}

#[test]
fn a_lone_backtick_is_written_with_two_backticks() {
    // `` ` `` → 内容是单个反引号
    assert_eq!(
        render("`` ` ``"),
        "<p class=\"nm-p\"><code class=\"nm-code-inline\">`</code></p>"
    );
    // 要包住两个反引号就得用三个
    assert_eq!(
        render("``` `` ```"),
        "<p class=\"nm-p\"><code class=\"nm-code-inline\">``</code></p>"
    );
}

#[test]
fn an_escaped_quote_stays_a_straight_quote() {
    // 转义的意义就是原样，所以它不该被智能标点接手
    assert_eq!(render("\\\"原文\\\""), "<p class=\"nm-p\">\"原文\"</p>");
    // 没转义就照常变成弯引号
    assert_eq!(render("\"原文\""), "<p class=\"nm-p\">“原文”</p>");
}

#[test]
fn a_lone_dollar_sign_is_written_with_two_dollars() {
    // 与代码跨度同一套逻辑：`$$ $ $$` 的内容是一个 `$`
    assert_eq!(
        render("$$ $ $$"),
        "<p class=\"nm-p\"><span class=\"nm-math\">\\($\\)</span></p>"
    );
    // 对比：同一段文本用反引号包起来就是代码
    assert_eq!(
        render("`` ` ``"),
        "<p class=\"nm-p\"><code class=\"nm-code-inline\">`</code></p>"
    );
}

#[test]
fn names_keys_and_values_may_all_be_quoted() {
    // 名字、键、值、链接目标四处共用同一套引号规则（这里看名字与键）。
    let html = render_with("::\"my name\" \"k 1\"=v", |registry| {
        registry.register("my name", EchoSignature);
    });

    assert_eq!(html, "<p class=\"nm-p\">[my name k 1=v]</p>");
}

#[test]
fn headings_are_registered_through_a_wildcard_pattern() {
    // `^h[1-6]$` 一条正则覆盖 h1~h6；正文走自然块展开器的**可调用接口**，
    // 所以标题里能写行内标记，而且不会多套一层段落。
    assert_eq!(
        render("::h1: 一级**标题**"),
        "<h1 class=\"nm-h1\">一级<strong class=\"nm-strong\">标题</strong></h1>"
    );
    assert_eq!(render("::h6: 六级"), "<h6 class=\"nm-h6\">六级</h6>");
}

#[test]
fn a_name_that_is_not_a_heading_falls_through_to_the_generic_error() {
    // 正则把级别写准了，所以 `ha` 根本不命中标题模式，落到兜底展开器上——
    // 报错由兜底给出，比标题展开器自己挡更准确，也不依赖展开器自觉。
    let html = render("::ha: x");
    assert!(html.contains("nm-error-no-handler"), "{html}");
    assert!(html.contains("未注册的调用块 ::ha"), "{html}");
}

#[test]
fn a_wrapper_registration_puts_the_body_inside_an_element() {
    // 这就是「注册时的语法糖」：一行注册，替代一整个 Handler 实现。
    let html = render_with("::notice: **注意**", |registry| {
        registry.register("notice", handlers::Wrap::tag("div").class("nm-notice"));
    });

    assert_eq!(
        html,
        concat!(
            "<div class=\"nm-notice\">",
            "<p class=\"nm-p\"><strong class=\"nm-strong\">注意</strong></p>",
            "</div>"
        )
    );
}

#[test]
fn a_wrapper_can_derive_a_class_from_the_call_name() {
    let html = render_with("::note-warning: 小心", |registry| {
        registry.register(
            Regex::new("^note-").unwrap(),
            handlers::Wrap::tag("aside").class("note").class_from_name(),
        );
    });

    assert_eq!(
        html,
        "<aside class=\"note note-warning\"><p class=\"nm-p\">小心</p></aside>"
    );
}

#[test]
fn a_wrapper_still_expands_nested_blocks() {
    // 块体是移交过去的，所以内层照样展开
    let html = render_with("::box:\n  ::h2: 标题", |registry| {
        registry.register("box", handlers::Wrap::tag("section").class("box"));
    });

    assert_eq!(
        html,
        "<section class=\"box\"><h2 class=\"nm-h2\">标题</h2></section>"
    );
}

#[test]
fn a_wrapper_can_read_capture_groups() {
    // 捕获组也是能拿到的：`::badge-new` 的样式由 `(?P<kind>…)` 决定，
    // 它匹配到的是 `new`（不是整名 `badge-new`）。
    let html = render_with("::badge-new: 新", |registry| {
        registry.register(
            Regex::new("^badge-(?P<kind>[a-z]+)$").unwrap(),
            handlers::Wrap::tag("span")
                .class("badge")
                .class_from(|m: &Matched| m.capture_named("kind").unwrap_or_default().to_string())
                .inline(handlers::NaturalExpander::default()),
        );
    });

    assert_eq!(html, "<span class=\"badge new\">新</span>");
}

#[test]
fn an_unanchored_pattern_can_tell_what_actually_matched() {
    // 模式只锚了中间：`h[1-6]` 会命中 `xh3y`，此时**整名**（xh3y）与
    // **实际匹配到的片段**（h3）不是一回事。
    let html = render_with("::xh3y: 正文", |registry| {
        registry.register(
            Regex::new("h[1-6]").unwrap(),
            handlers::Wrap::tag_from(|m: &Matched| m.matched().to_string())
                .inline(handlers::NaturalExpander::default()),
        );
    });

    // 标签取实际匹配到的 `h3`，而不是整名
    assert_eq!(html, "<h3>正文</h3>");
}

#[test]
fn headings_really_are_just_a_wrapper_now() {
    // 与 `headings_are_registered_through_a_wildcard_pattern` 一对照：输出完全
    // 相同，但内置实现里已经没有任何专门的标题展开器了。
    assert_eq!(render("::h3: 三级"), "<h3 class=\"nm-h3\">三级</h3>");
}

#[test]
fn a_wrapper_can_use_several_capture_groups_at_once() {
    // 多个捕获组：按名字各取一个，拼成一个类名；缺的用兜底值补齐。
    let html = render_with("::figure-chart-v2: 正文", |registry| {
        registry.register(
            Regex::new(r"^figure-(?P<kind>\w+)-v(?P<version>\d+)$").unwrap(),
            handlers::Wrap::tag("figure")
                .class("figure")
                .class_from(|m: &Matched| {
                    format!(
                        "{}-v{}",
                        m.capture_named("kind").unwrap_or("unknown"),
                        m.capture_named("version").unwrap_or("0"),
                    )
                })
                .inline(handlers::NaturalExpander::default()),
        );
    });

    assert_eq!(html, "<figure class=\"figure chart-v2\">正文</figure>");
}

#[test]
fn the_common_derivations_need_no_closure_at_all() {
    // `tag_from_match` + `class_prefix` + `class_from_match` 覆盖了「标签与类名
    // 都取自匹配片段」这个最常见的情形，不用写闭包、也不用标类型。
    let html = render_with("::h2: 标题", |registry| {
        registry.register(
            Regex::new("^h[1-6]$").unwrap(),
            handlers::Wrap::tag_from_match()
                .class_prefix("nm-")
                .class_from_match()
                .inline(handlers::NaturalExpander::default()),
        );
    });

    assert_eq!(html, "<h2 class=\"nm-h2\">标题</h2>");
}
#[test]
fn an_instance_produced_inline_becomes_a_span_not_a_div() {
    // 行内调用展开出的模板实例落在 <p> 里面：必须是 <span>，
    // 否则就是 <p><div>…</div></p>——非法 HTML。
    let html = render_with("看 {{badge: 新}} 这里", |registry| {
        registry.register("badge", InlineBadge);
    });

    assert!(
        html.contains("<span class=\"nm-instance nm-instance-badge\""),
        "{html}"
    );
    assert!(!html.contains("<div"), "{html}");
}

#[test]
fn a_block_only_expander_cannot_leak_a_block_element_inline() {
    // `^h[1-6]$` 注册的是**块级**包装器（只实现了 expand_call）。行内调用命中
    // 同一个展开器时会走 expand_inline，而它没实现 → 行内报错，绝不会产出 <h2>。
    let html = render("看 {{h2: 标题}} 这里");

    assert!(html.contains("nm-error-inline"), "{html}");
    assert!(!html.contains("<h2"), "{html}");
    assert!(!html.contains("<div"), "{html}");
}

#[test]
fn the_first_line_content_is_trimmed_on_both_ends() {
    let html = render_with("::echo   \"k\"=v   ", |registry| {
        registry.register("echo", EchoSignature);
    });

    assert_eq!(html, "<p class=\"nm-p\">[echo k=v]</p>");
}
#[test]
fn a_block_element_from_an_inline_call_breaks_the_paragraph() {
    // 行内调用展开出块级元素时，段落被**打断**，而不是产出 <p><div></p>。
    let html = render_with("看 {{box: 内容}} 这里", |registry| {
        registry.register("box", InlineDiv);
    });

    assert_eq!(
        html,
        concat!(
            "<p class=\"nm-p\">看 </p>",
            "<div class=\"box\">内容</div>",
            "<p class=\"nm-p\"> 这里</p>",
        )
    );
}

#[test]
fn a_hard_break_uses_a_backslash_but_a_soft_break_stays_a_newline() {
    assert_eq!(render("硬\\\n换行"), "<p class=\"nm-p\">硬<br>换行</p>");
    assert_eq!(render("软\n换行"), "<p class=\"nm-p\">软\n换行</p>");
}

#[test]
fn a_call_block_still_swallows_its_subtree_into_one_box() {
    // 没有 ::code 展开器时，::code 整块（连同内部嵌套的块）变成一个提示框，
    // 内部不会再各自爆出提示框。
    let source = "::code lang=neomark:\n  ::notice:\n    内层 **没有** 行内解析\n";
    let html = render(source);

    assert_eq!(html.matches("nm-error-no-handler").count(), 1);
    // 提示框里是原样回显，不做行内解析
    assert!(html.contains("内层 **没有** 行内解析"));
}

#[test]
fn links_render_end_to_end() {
    assert_eq!(
        render("看 [[**粗体** 文本 => https://a.com]] 这里"),
        concat!(
            "<p class=\"nm-p\">看 ",
            "<a class=\"nm-link\" href=\"https://a.com\">",
            "<strong class=\"nm-strong\">粗体</strong> 文本",
            "</a> 这里</p>"
        )
    );
}

#[test]
fn the_last_arrow_wins_so_targets_may_follow_text_containing_arrows() {
    assert_eq!(
        render("[[a => b => /x]]"),
        "<p class=\"nm-p\"><a class=\"nm-link\" href=\"/x\">a =&gt; b</a></p>"
    );
}

#[test]
fn a_link_inside_a_code_span_stays_literal() {
    // 代码跨度里的 `>` 照样要 HTML 转义，所以看到的是 =&gt;
    assert_eq!(
        render("`[[a => b]]`"),
        "<p class=\"nm-p\"><code class=\"nm-code-inline\">[[a =&gt; b]]</code></p>"
    );
}

#[test]
fn a_malformed_link_stays_literal_text() {
    assert_eq!(render("[[没有箭头]]"), "<p class=\"nm-p\">[[没有箭头]]</p>");
}
