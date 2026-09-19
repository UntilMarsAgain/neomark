//! 行内层：把自然块的文本解析成**语义**节点。
//!
//! # 支持的构造
//!
//! | 构造 | 语法 | AST 里的语义 |
//! |---|---|---|
//! | 反斜杠转义 | `\*` | 字面字符 |
//! | 硬换行 | 行尾 `\` | [`NodeKind::LineBreak`] |
//! | 软换行 | 裸换行 | 文本里的换行 |
//! | 代码跨度 | `` `code` ``、`` ``a`b`` `` | [`NodeKind::Code`] + 原样文本 |
//! | 数学 | `$x$`、`$$a$b$$` | [`NodeKind::Math`] + 原样文本 |
//! | 实体 | `&amp;` `&#35;` | 对应字符 |
//! | 行内调用 · 糖形态 | `:smile:` | [`NodeKind::InlineCall`]，等价于 `{{smile}}` |
//! | 行内调用 · 全形 | `{{name k=v: 内容}}` | [`NodeKind::InlineCall`] + 未解析的内容块 |
//! | 链接 | `[[文本 => 目标]]` | [`NodeKind::Link`] + 未解析的文本块 |
//! | 强调 / 加粗 | `*x*` / `**x**` | [`NodeKind::Emphasis`] / [`NodeKind::Strong`] |
//! | 删除线 | `~~x~~` | [`NodeKind::Strikethrough`] |
//! | 下标 / 上标 | `~x~` / `^x^` | [`NodeKind::Subscript`] / [`NodeKind::Superscript`] |
//! | 高亮 | `==x==` | [`NodeKind::Mark`] |
//! | 智能标点 | `...` `--` `---` `"x"` | `…` `–` `—` `“x”` |
//!
//! **这里一个 HTML 字符串都没有。** 标签、类名、`\(...\)` 包装、emoji 到底
//! 长什么样，全是 [`crate::html`] 的决定。
//!
//! # 行内调用与块调用共用同一套头部语法
//!
//! `{{name k=v: 内容}}` 的括号内部**直接交给 [`crate::parse::parse_call_header`]**
//! ——和 `::name k=v: 内容` 是同一个解析器。所以「括号内部与块头部逐字节相同」
//! 不是靠约定，而是靠代码本身成立的；`url=http://x` 这类含冒号的值在两边
//! 行为完全一致。
//!
//! # 刻意不做的事
//!
//! * **`_` 不是强调**。只保留 `*` 定义的强调与加粗，`_` 一律当普通字符。
//! * **没有行内原始 HTML**。`<div>` 会被当成普通文本转义输出。
//! * **不查任何名字表**。行内调用只带走名字与参数，认不认识是渲染器的
//!   事——这样将来换成手搓 SVG、加属性，解析层不用动。
//! * **实体保留在解析层解析**。`&copy;` 的语义就是那个字符本身，不是一种
//!   呈现方式，跟行内调用不同。
//!
//! # 已知取舍
//!
//! * 数学按需求「包裹行为与反引号一致」：靠**游程长度相等**配对，没有
//!   「内侧不能有空白」的守卫。所以正文里孤立的两个 `$` 会配成公式
//!   （`$5 和 $10` 会被当成 `5 和 `）。要 Pandoc 那种守卫，在
//!   [`scan`] 的 `$` 分支加一条判断即可。
//! * 命名实体是**常用子集**，查不到的按字面保留。
//! * 标点判定用 ASCII 标点近似 CommonMark 的 Unicode `P*` 类别。
//! * 行内调用与链接的**内容**都被包成未解析自然块，而位置用的是外层块的
//!   span——行内层还没有列偏移。

mod delim;
mod entities;
mod scan;
mod smart;

use crate::ast::{Ast, Block, NaturalBlock, NodeId, NodeKind, Params, Span};

/// 行内层的中间表示，只在行内层内部使用，不进入 arena。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Piece {
    /// 普通文本。
    Text(String),
    /// 已经定型的语义节点，孩子还没降级成节点。
    Node {
        kind: NodeKind,
        children: Vec<Piece>,
    },
    /// 行内调用：名字 + 参数 + 可选的**首行内容**。
    ///
    /// 参数保持字符串；**要不要对某个参数值做行内解析，由展开器自己决定**
    /// （展开器拿到 `Params`，可以自己调 [`parse`] 把某个值变成行内内容）。
    InlineCall {
        name: String,
        params: Params,
        content: Option<String>,
    },
    /// 链接：文本**还没展开**，由调度器当成自然块照常展开。
    Link { target: String, text: String },
    /// 硬换行。
    LineBreak,
    /// 尚未配对的分隔符游程。
    Delim {
        ch: char,
        count: usize,
        can_open: bool,
        can_close: bool,
    },
}

/// 解析一段行内文本，把结果挂进 `ast`，返回顶层节点（按文档顺序）。
///
/// 调用方负责把它们挂到合适的位置（例如段落下）。
///
/// `span` 是这段文本在原文里的位置。行内层目前**不跟踪列偏移**，所以链接
/// 文本被包成未解析自然块时用的是这个位置（即外层块的位置）——偏大但有效。
pub fn parse(ast: &mut Ast, text: &str, span: Span) -> Vec<NodeId> {
    lower(ast, delim::resolve(scan::scan(text)), span)
}

fn lower(ast: &mut Ast, pieces: Vec<Piece>, span: Span) -> Vec<NodeId> {
    pieces
        .into_iter()
        .map(|piece| lower_one(ast, piece, span))
        .collect()
}

fn lower_one(ast: &mut Ast, piece: Piece, span: Span) -> NodeId {
    match piece {
        Piece::Text(text) => ast.new_text(text),
        Piece::LineBreak => ast.new_line_break(),
        Piece::InlineCall {
            name,
            params,
            content,
        } => {
            let call = ast.new_inline_call(name, params, span);
            if let Some(content) = content {
                // 首行内容先是一个**未解析的自然块**，交给调度器照常展开。
                // 于是 `{{quote: **粗体**}}` 里的行内标记不需要任何新机制。
                let inner = ast.new_node(NodeKind::Unparsed(Block::Natural(NaturalBlock {
                    text: content,
                    span,
                    inline: true,
                })));
                ast.append(call, inner);
            }
            call
        }
        Piece::Link { target, text } => {
            let link = ast.new_link(target);
            // 链接文本先是一个**未解析的自然块**，交给调度器照常展开。
            // 这样链接里写粗体、代码都不用任何新机制。
            let inner = ast.new_node(NodeKind::Unparsed(Block::Natural(NaturalBlock {
                text,
                span,
                inline: true,
            })));
            ast.append(link, inner);
            link
        }
        Piece::Node { kind, children } => {
            let node = ast.new_node(kind);
            for child in lower(ast, children, span) {
                ast.append(node, child);
            }
            node
        }
        // 没配上对的分隔符落回字面文本。
        Piece::Delim { ch, count, .. } => ast.new_text(ch.to_string().repeat(count)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 走完整链路（行内层 + 调度器）再渲染，然后剥掉外层段落。
    ///
    /// 必须包一层段落：渲染器在**顶层**块之间插换行，直接当顶层节点会凭空
    /// 多出换行，而换行正是软换行测试要区分的东西。
    ///
    /// 也必须跑调度器：链接文本在 AST 里是一个**未解析的自然块**，要由调度器
    /// 展开——这正是「链接里能写粗体」的来源。
    fn html(text: &str) -> String {
        let mut ast = Ast::new();
        let wrapper = ast.new_paragraph();
        for id in parse(&mut ast, text, Span::new(1, 1, 0, text.len())) {
            ast.append(wrapper, id);
        }
        ast.push_block(wrapper);

        let mut registry = crate::dispatch::Registry::new();
        crate::handlers::register_defaults(&mut registry);
        let mut ctx = crate::dispatch::Context::new(text);
        crate::dispatch::Dispatcher::new(registry).run(&mut ast, &mut ctx);

        crate::html::render(&ast)
            .strip_prefix("<p class=\"nm-p\">")
            .and_then(|inner| inner.strip_suffix("</p>"))
            .expect("渲染器输出的就是这一层段落")
            .to_string()
    }

    #[test]
    fn escapes_and_hard_breaks() {
        assert_eq!(html("\\*不是强调\\*"), "*不是强调*");
        assert_eq!(html("上\\\n下"), "上<br>下");
        // 软换行原样保留，交给 HTML 折叠
        assert_eq!(html("上\n下"), "上\n下");
    }

    #[test]
    fn emphasis_and_strong_use_asterisks_only() {
        assert_eq!(html("*a*"), "<em class=\"nm-em\">a</em>");
        assert_eq!(html("**a**"), "<strong class=\"nm-strong\">a</strong>");
        assert_eq!(
            html("***a***"),
            "<em class=\"nm-em\"><strong class=\"nm-strong\">a</strong></em>"
        );
        // 下划线不是强调
        assert_eq!(html("_a_"), "_a_");
        assert_eq!(html("__a__"), "__a__");
    }

    #[test]
    fn code_spans_are_verbatim() {
        assert_eq!(html("`*a*`"), "<code class=\"nm-code-inline\">*a*</code>");
        assert_eq!(html("``a`b``"), "<code class=\"nm-code-inline\">a`b</code>");
        assert_eq!(
            html("`&amp;`"),
            "<code class=\"nm-code-inline\">&amp;amp;</code>"
        );
    }

    #[test]
    fn math_is_marked_as_math_and_the_renderer_wraps_it() {
        // 归一成 KaTeX 默认认的 `\(...\)` 是渲染器的决定，不是解析层的。
        assert_eq!(html("$x^2$"), "<span class=\"nm-math\">\\(x^2\\)</span>");
        assert_eq!(html("$$a$b$$"), "<span class=\"nm-math\">\\(a$b\\)</span>");
    }

    #[test]
    fn entities_are_resolved_but_inline_call_names_are_not() {
        assert_eq!(html("&amp;"), "&amp;");
        assert_eq!(html("&#35;"), "#");
        assert_eq!(html("&hellip;"), "…");
        // 名字只带走，长什么样是渲染器查出来的
        assert_eq!(
            html(":smile:"),
            "<span class=\"nm-emoji\" data-alias=\"smile\">😄</span>"
        );
        // 糖形态与全形在这里合流
        assert_eq!(
            html("{{smile}}"),
            "<span class=\"nm-emoji\" data-alias=\"smile\">😄</span>"
        );
        // 渲染器不认识的名字，原样回显
        assert_eq!(html(":nope:"), ":nope:");
        assert_eq!(html("12:30:45"), "12:30:45");
    }

    #[test]
    fn a_braced_inline_call_carries_params_and_content() {
        // 内容走既有的展开管线，所以行内标记照常生效；
        // 没有展开器的名字由渲染器原样回显，但内容不会被吞掉。
        assert_eq!(
            html("{{quote author=张三: **粗体**}}"),
            "{{quote author=张三: <strong class=\"nm-strong\">粗体</strong>}}"
        );
    }

    #[test]
    fn strikethrough_subscript_superscript_highlight() {
        assert_eq!(html("~~a~~"), "<del class=\"nm-del\">a</del>");
        assert_eq!(html("~a~"), "<sub class=\"nm-sub\">a</sub>");
        assert_eq!(html("2^10^"), "2<sup class=\"nm-sup\">10</sup>");
        assert_eq!(html("==a=="), "<mark class=\"nm-mark\">a</mark>");
    }

    #[test]
    fn smart_punctuation() {
        assert_eq!(html("等一下..."), "等一下…");
        assert_eq!(html("2010--2020"), "2010–2020");
        assert_eq!(html("他说\"好\""), "他说“好”");
        assert_eq!(html("don't"), "don’t");
    }

    #[test]
    fn inline_html_is_plain_text() {
        // 按当前决定：不解析行内 HTML，原样转义成文本。
        assert_eq!(html("<div>hi</div>"), "&lt;div&gt;hi&lt;/div&gt;");
        assert_eq!(html("<span>a</span>"), "&lt;span&gt;a&lt;/span&gt;");
    }

    #[test]
    fn smart_punctuation_also_touches_html_lookalikes() {
        // 行内 HTML 既然一律当文本，智能标点自然也会作用在它上面——
        // 这是「不做行内 HTML」的一个附带后果，写下来免得以后当 bug 查。
        assert_eq!(html("<a href=\"x\">y</a>"), "&lt;a href=“x”&gt;y&lt;/a&gt;");
    }

    #[test]
    fn nesting_works_across_constructs() {
        assert_eq!(
            html("**粗 *斜* 体**"),
            "<strong class=\"nm-strong\">粗 <em class=\"nm-em\">斜</em> 体</strong>"
        );
        assert_eq!(
            html("*a `b` c*"),
            "<em class=\"nm-em\">a <code class=\"nm-code-inline\">b</code> c</em>"
        );
    }

    #[test]
    fn links_render_with_their_target() {
        assert_eq!(
            html("[[文本 => https://a.com]]"),
            "<a class=\"nm-link\" href=\"https://a.com\">文本</a>"
        );
    }

    #[test]
    fn link_text_goes_through_the_normal_pipeline() {
        // 链接文本是未解析的自然块，所以粗体、代码照常生效——
        // 这是复用既有展开管线换来的，没有为链接新写一套行内解析。
        assert_eq!(
            html("[[**粗** 和 `码` => /x]]"),
            concat!(
                "<a class=\"nm-link\" href=\"/x\">",
                "<strong class=\"nm-strong\">粗</strong> 和 ",
                "<code class=\"nm-code-inline\">码</code>",
                "</a>"
            )
        );
    }

    #[test]
    fn link_text_is_not_wrapped_in_a_block() {
        // 行内上下文里不该冒出块级段落
        assert!(!html("[[文本 => /x]]").contains("<p"));
    }

    #[test]
    fn link_targets_are_escaped() {
        assert_eq!(
            html("[[a => x\"y&z]]"),
            "<a class=\"nm-link\" href=\"x&quot;y&amp;z\">a</a>"
        );
    }

    #[test]
    fn one_link_never_contains_another() {
        // 区域由第一个 `]]` 界定，所以嵌套链接在语法上就不可能出现
        let rendered = html("[[a [[b => c]] => d]]");
        assert_eq!(rendered.matches("<a class=").count(), 1);
    }
}
