//! 端到端：源文本 → 解析 → 展开 → HTML。
//!
//! 这里只用公共 API，模拟外部使用者。

use neomark::{Context, Dispatcher, Registry, handlers, html, parse};

fn render(source: &str) -> String {
    let mut ast = parse(source);
    let mut registry = Registry::new();
    handlers::register_defaults(&mut registry);

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
