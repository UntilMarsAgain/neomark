//! 展开器接口与展开上下文。

use super::matched::Matched;
use crate::ast::{Ast, ErrorKind, ErrorNode, KindTag, NodeId, Span};

/// 一个块展开器。
///
/// 展开器拿到的是**节点 id**（`Copy`），不是借用——这样「读载荷」和「改树」
/// 不会互相打架：每次访问都是短暂借用，全程不存在同时持有的可变借用。
///
/// 自然块和调用块走同一条分发路径：[`Handler::expand`] 按种类分发到
/// [`Handler::expand_call`] / [`Handler::expand_natural`]，后两者都有默认
/// 实现，默认行为是产出一个「没有展开器认领」的报错节点。
///
/// 返回值是**替换该节点的兄弟序列**：返回空表示整块丢弃，返回一个表示替换成
/// 一个节点，返回多个表示展开成多个兄弟节点。返回的节点里若仍含未展开的块，
/// 调度器会继续推进。
///
/// **按名分发的那两个方法会收到 [`Matched`]**：里面有完整调用名、正则实际匹配
/// 到的那一段，以及各捕获组。自然块没有名字，所以
/// [`expand_natural`](Handler::expand_natural) 不收这个参数。
///
/// # 在展开器里拿捕获组
///
/// 注册用 `register_pattern`，取组用 [`Matched`] 的三个读法。**多个捕获组**按
/// 名字取最稳（模式里改组的顺序也炸不了），按下标取省事，整组遍历用
/// [`Matched::groups`]：
///
/// ```
/// # use neomark::{Ast, Context, Dispatcher, Handler, Matched, NodeId, Registry, parse};
/// # use neomark::regex::Regex;
/// /// `::figure-chart-v2` → 把两个捕获组写进参数
/// struct Figure;
///
/// impl Handler for Figure {
///     fn expand_call(
///         &self,
///         node: NodeId,
///         ast: &mut Ast,
///         _ctx: &mut Context<'_>,
///         matched: &Matched<'_>,
///     ) -> Vec<NodeId> {
///         // ① 按名字取（推荐）
///         let kind = matched.capture_named("kind").unwrap_or("unknown");
///         // ② 按下标取：0 是整段匹配，所以第一个括号是 1
///         let version = matched.capture(2).unwrap_or("0");
///         // ③ 整组遍历：含未参与匹配的 None
///         assert_eq!(matched.group_count(), 3);
///         assert_eq!(
///             matched.groups().collect::<Vec<_>>(),
///             vec![Some("figure-chart-v2"), Some("chart"), Some("2")],
///         );
///
///         let mut params = ast.call(node).unwrap().params.clone();
///         params.push("kind", kind);
///         params.push("version", version);
///
///         let instance = ast.new_instance("figure", params);
///         for child in ast.children(node).collect::<Vec<_>>() {
///             ast.append(instance, child);
///         }
///         vec![instance]
///     }
/// }
///
/// let mut registry = Registry::new();
/// registry.register(
///     Regex::new(r"^figure-(?P<kind>\w+)-v(?P<version>\d+)$").unwrap(),
///     Figure,
/// );
///
/// let source = "::figure-chart-v2: 正文";
/// let mut ast = parse(source);
/// let mut ctx = Context::new(source);
/// Dispatcher::new(registry).run(&mut ast, &mut ctx);
///
/// let html = neomark::html::render(&ast);
/// assert!(html.contains("data-kind=\"chart\""), "{html}");
/// assert!(html.contains("data-version=\"2\""), "{html}");
/// ```
///
/// 需要正则原生的 `Captures`（下标、`expand`、按名字定位）时用
/// [`Matched::raw`]。
///
/// # 精确名注册时呢
///
/// 精确名命中没有捕获组，但读法都还能用，只是退化成同一个值：`name()` 与
/// `matched()` 都是调用名，`capture(0)` 也返回它，`capture(1)` 是 `None`。
/// 所以展开器可以照常写，不必区分自己是怎么被注册的。
pub trait Handler {
    /// 展开一个未展开的节点。默认按种类分发到下面三个方法。
    fn expand(
        &self,
        node: NodeId,
        ast: &mut Ast,
        ctx: &mut Context<'_>,
        matched: &Matched<'_>,
    ) -> Vec<NodeId> {
        match ast.tag(node) {
            Some(KindTag::Call) => self.expand_call(node, ast, ctx, matched),
            Some(KindTag::InlineCall) => self.expand_inline(node, ast, ctx, matched),
            Some(KindTag::Natural) => self.expand_natural(node, ast, ctx),
            _ => Vec::new(),
        }
    }

    /// 展开一个调用块。默认产出报错节点。
    fn expand_call(
        &self,
        node: NodeId,
        ast: &mut Ast,
        ctx: &mut Context<'_>,
        _matched: &Matched<'_>,
    ) -> Vec<NodeId> {
        let Some(call) = ast.call(node) else {
            return Vec::new();
        };
        let span = call.span;
        let name = call.name.clone();

        vec![ast.new_error(ErrorNode::new(
            ErrorKind::NoHandler,
            format!("未注册的调用块 ::{name}"),
            span,
            ctx.slice(span),
        ))]
    }

    /// 展开一个**行内调用**。默认产出报错节点。
    ///
    /// 注意调度器**只在名字有注册展开器时**才会走到这里。名字没有展开器的
    /// 行内调用会保持原样，交给渲染器按名字解释——emoji 走的就是那条路。
    fn expand_inline(
        &self,
        node: NodeId,
        ast: &mut Ast,
        _ctx: &mut Context<'_>,
        _matched: &Matched<'_>,
    ) -> Vec<NodeId> {
        let Some((name, params, span)) = ast.inline_call(node) else {
            return Vec::new();
        };
        let name = name.to_string();
        // 行内没有精确列位置，`ctx.slice(span)` 会给出整个外块——太宽。所以这里用
        // 规范形式**重建**这段调用（引号规则与解析层同一套），渲染器再把它当作
        // 「出错的那段原文」显示出来。
        let mut source = String::from("{{");
        source.push_str(&crate::parse::format_call_header(&name, params));
        source.push_str("}}");

        vec![
            ast.new_error(
                ErrorNode::new(
                    ErrorKind::NoHandler,
                    format!("没有展开器能处理行内调用 {{{name}}}"),
                    span,
                    source,
                )
                .at_inline(),
            ),
        ]
    }

    /// 展开一个自然块。默认产出报错节点。
    fn expand_natural(&self, node: NodeId, ast: &mut Ast, ctx: &mut Context<'_>) -> Vec<NodeId> {
        let Some(natural) = ast.natural(node) else {
            return Vec::new();
        };
        let span = natural.span;

        vec![ast.new_error(ErrorNode::new(
            ErrorKind::NoHandler,
            "没有注册自然块展开器",
            span,
            ctx.slice(span),
        ))]
    }
}

/// 兜底展开器：没有展开器认领这个块时使用。
///
/// 它**就是 [`Handler`] 的全部默认行为**——把块的原文内容原样放进报错节点，
/// 再由渲染器决定怎么显示。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Fallback;

impl Handler for Fallback {}

/// 展开过程中传给展开器的上下文。
pub struct Context<'a> {
    source: &'a str,
}

impl<'a> Context<'a> {
    /// 用原始输入构造上下文。
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    /// 原始输入。
    pub fn source(&self) -> &'a str {
        self.source
    }

    /// 按位置区间从原始输入切片。
    ///
    /// 展开器想拿到某个块的**原样文本**（含头部行、含原始缩进）时用它；
    /// 只要块体本身就用 [`crate::ast::CallBlock::raw_body`]。
    pub fn slice(&self, span: Span) -> &'a str {
        span.slice(self.source)
    }
}
