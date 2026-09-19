//! 行内层的配置。

/// 行内层的配置：每一项都可以单独关掉。
///
/// 默认**全开**。全部关掉时行内层退化成「原样文本」——`::code` 这类要原文的
/// 场合就是这个意思；外部解析器也可以只挑自己认识的那几项。
///
/// 这是 [`crate::inline::parse`] 的配置入口。展开器拿到参数（字符串）之后要
/// 自己决定怎么解析时，就在这儿挑选项：
///
/// ```
/// use neomark::inline::{parse, Options};
/// # let mut ast = neomark::Ast::new();
/// # let span = neomark::Span::new(1, 1, 0, 0);
/// // 只要强调与代码，不要链接、行内调用、智能标点
/// let options = Options {
///     links: false,
///     calls: false,
///     smart_punctuation: false,
///     ..Options::default()
/// };
/// let nodes = parse(&mut ast, "**粗** 还是 *斜*", span, options);
/// # assert_eq!(nodes.len(), 3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// 反斜杠转义与行尾硬换行。
    pub escapes: bool,
    /// 代码跨度：`` `x` ``。
    pub code_spans: bool,
    /// 数学：`$x$`。
    pub math: bool,
    /// 实体：`&amp;`。
    pub entities: bool,
    /// 行内调用：`{{…}}` 与糖形态 `:name:`。
    pub calls: bool,
    /// 链接：`[[文本 => 目标]]`。
    pub links: bool,
    /// 强调与加粗：`*x*` / `**x**`。
    pub emphasis: bool,
    /// 删除线、下标、上标、高亮：`~~x~~` `~x~` `^x^` `==x==`。
    pub styles: bool,
    /// 智能标点：`...` `--` `"x"`。
    pub smart_punctuation: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            escapes: true,
            code_spans: true,
            math: true,
            entities: true,
            calls: true,
            links: true,
            emphasis: true,
            styles: true,
            smart_punctuation: true,
        }
    }
}

impl Options {
    /// 全关：行内层退化成原样文本。
    pub const fn none() -> Self {
        Self {
            escapes: false,
            code_spans: false,
            math: false,
            entities: false,
            calls: false,
            links: false,
            emphasis: false,
            styles: false,
            smart_punctuation: false,
        }
    }

    /// 是否一项都没开（行内层等于原样文本）。
    pub const fn is_plain(&self) -> bool {
        !self.escapes
            && !self.code_spans
            && !self.math
            && !self.entities
            && !self.calls
            && !self.links
            && !self.emphasis
            && !self.styles
            && !self.smart_punctuation
    }
}
