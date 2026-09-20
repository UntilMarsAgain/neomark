//! 调度器：把未展开的块推进到不动点。

use super::handler::Context;
use super::matched::Matched;
use super::registry::Registry;
use crate::ast::{Ast, KindTag, NodeId};

/// 反复展开未展开的块，直到树中不再有 [`KindTag::Call`] / [`KindTag::Natural`]。
///
/// # 分发
///
/// 调用块按调用名查找展开器，自然块查找自然块展开器；**两者都没命中时用
/// 兜底展开器**——默认的 [`Fallback`](super::Fallback) 会把块的原文内容
/// 放进报错节点，因此任何块都不会被静默丢弃。
///
/// # 展开顺序
///
/// 自上而下：先展开当前未展开节点，再递归处理**替换结果**里的节点。
/// 展开器对自己原有的子节点是否被访问有完全的控制权——把子节点搬进替换
/// 子树，它们才会被继续展开。
///
/// # 终止性
///
/// 改写规则不保证收敛——如果某个展开器展开出的子树里又出现同一个块，
/// 这里会一直展开下去。**当前版本还没有预算/深度限制**；将来接入预算的
/// 位置就是下面 `expand_node` 里调用展开器之前。
pub struct Dispatcher {
    registry: Registry,
}

impl Dispatcher {
    /// 用一个注册表构造调度器。
    pub fn new(registry: Registry) -> Self {
        Self { registry }
    }

    /// 只读访问注册表。
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// 把整棵树的顶层块展开到不动点。
    pub fn run(&self, ast: &mut Ast, ctx: &mut Context<'_>) {
        let document = ast.document();
        let roots: Vec<NodeId> = ast.children(document).collect();
        for root in roots {
            self.expand_node(ast, root, ctx);
        }
    }

    /// 展开一个节点，并递归处理它展开出来的节点。
    fn expand_node(&self, ast: &mut Ast, node: NodeId, ctx: &mut Context<'_>) {
        // 行内调用走单独一条路：**没有展开器就保持原样**，不套兜底报错。
        if ast.tag(node) == Some(KindTag::InlineCall) {
            self.expand_inline_call(ast, node, ctx);
            return;
        }

        if !ast.is_unparsed(node) {
            // 已展开的节点：继续往下走。
            let children: Vec<NodeId> = ast.children(node).collect();
            for child in children {
                self.expand_node(ast, child, ctx);
            }
            return;
        }

        // 名字先复制成局部变量：`Captures` 借用 haystack，而 haystack 原本住在
        // `Ast` 里——展开器同时还要 `&mut Ast`，借用检查不允许两者并存。
        let name = match ast.tag(node) {
            Some(KindTag::Call) => ast.call(node).map(|call| call.name.clone()),
            _ => None,
        };

        let replacement = match name.as_deref().and_then(|name| self.registry.find(name)) {
            // 按名命中：把命中信息（含捕获组）交给展开器。
            Some(found) => {
                let matched = Matched::new(name.as_deref().unwrap_or_default(), found.captures);
                found.handler.expand(node, ast, ctx, &matched)
            }
            // 自然块与兜底：自然块没有名字。
            None => {
                let handler = match ast.tag(node) {
                    Some(KindTag::Natural) => self
                        .registry
                        .natural()
                        .unwrap_or_else(|| self.registry.fallback()),
                    _ => self.registry.fallback(),
                };
                handler.expand(node, ast, ctx, &Matched::unnamed())
            }
        };

        self.splice(ast, node, replacement, ctx);
    }

    /// 展开一个行内调用。
    ///
    /// 和块调用**一样**：名字没人认领就交给兜底展开器（默认产出报错节点）。
    /// 行内图标 `:name:` 走的是另一条路——它是呈现，压根不进调度器，由渲染器
    /// 查表。
    fn expand_inline_call(&self, ast: &mut Ast, node: NodeId, ctx: &mut Context<'_>) {
        let name = ast.inline_call(node).map(|(name, _, _)| name.to_string());

        let (handler, captures) = match name.as_deref().and_then(|name| self.registry.find(name)) {
            Some(found) => (found.handler, found.captures),
            None => (self.registry.fallback(), None),
        };

        let matched = Matched::new(name.as_deref().unwrap_or_default(), captures);
        let replacement = handler.expand(node, ast, ctx, &matched);
        self.splice(ast, node, replacement, ctx);
    }

    /// 用 `replacement` 在原位替换掉 `node`，再递归处理替换结果。
    fn splice(&self, ast: &mut Ast, node: NodeId, replacement: Vec<NodeId>, ctx: &mut Context<'_>) {
        // 把替换结果插在 node 原来的位置上，顺序保持不变。
        let mut anchor = node;
        for &next in &replacement {
            ast.insert_after(anchor, next);
            anchor = next;
        }
        // 再删掉 node 本身，以及**没有被搬运走**的原子树。`::code` 这类
        // 原样文本的展开器正是靠这一步让内层永远不会被展开。
        ast.remove_subtree(node);

        for next in replacement {
            self.expand_node(ast, next, ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_util::sexpr;
    use crate::ast::{Attr, ErrorKind, ErrorNode, Span};
    use crate::dispatch::Handler;
    use crate::handlers::NaturalExpander as RealNaturalExpander;
    use crate::parse::parse;

    /// 只关心调用块：把调用**换成一个模板实例**，并把原有子节点重新挂载过去。
    ///
    /// 注意它不碰任何 HTML——产出什么标签、什么类名是渲染器的事。
    struct NoticeHandler;

    impl Handler for NoticeHandler {
        fn expand_call(
            &self,
            node: NodeId,
            ast: &mut Ast,
            _ctx: &mut Context<'_>,
            _matched: &Matched<'_>,
        ) -> Vec<NodeId> {
            let (name, params) = {
                let call = ast.call(node).unwrap();
                (call.name.clone(), call.params.clone())
            };
            let instance = ast.new_instance(name, params);

            // 移交块体：一次 append 就把子节点搬过来了。
            for child in ast.children(node).collect::<Vec<_>>() {
                ast.append(instance, child);
            }
            vec![instance]
        }
    }

    /// 逃生口：展开器自己写 HTML 元素，而不是用语义节点。
    ///
    /// 外部插件常见这种写法——它不认识 neomark 的语义节点，只想输出自己
    /// 那套结构。渲染器会原样输出。
    struct RawElementHandler;

    impl Handler for RawElementHandler {
        fn expand_call(
            &self,
            node: NodeId,
            ast: &mut Ast,
            _ctx: &mut Context<'_>,
            _matched: &Matched<'_>,
        ) -> Vec<NodeId> {
            let aside = ast.new_element("aside", vec![Attr::new("class", "tip")]);
            for child in ast.children(node).collect::<Vec<_>>() {
                ast.append(aside, child);
            }
            vec![aside]
        }
    }

    /// 行内调用展开器：把 `{{badge …}}` 换成一个模板实例。
    ///
    /// 它拿到的参数是**字符串**，要不要对某个值做行内解析完全由它自己决定
    /// ——这里故意什么都不做，只把参数交给实例。
    struct BadgeHandler;

    impl Handler for BadgeHandler {
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

    /// 会把自己是谁写进输出，用来分辨命中了哪条注册。
    struct Marker(&'static str);

    impl Handler for Marker {
        fn expand_call(
            &self,
            node: NodeId,
            ast: &mut Ast,
            _ctx: &mut Context<'_>,
            _matched: &Matched<'_>,
        ) -> Vec<NodeId> {
            let name = ast.call(node).unwrap().name.clone();
            let text = ast.new_text(format!("{}:{name}", self.0));
            vec![text]
        }
    }

    /// 只关心自然块。
    struct ParagraphHandler;
    impl Handler for ParagraphHandler {
        fn expand_natural(
            &self,
            node: NodeId,
            ast: &mut Ast,
            _ctx: &mut Context<'_>,
        ) -> Vec<NodeId> {
            let text = ast.natural(node).unwrap().text.clone();

            let paragraph = ast.new_paragraph();
            let text = ast.new_text(text);
            ast.append(paragraph, text);
            vec![paragraph]
        }
    }

    /// 只取原文，**不搬运**原子节点：内层永远不会被展开。
    struct VerbatimHandler;

    impl Handler for VerbatimHandler {
        fn expand_call(
            &self,
            node: NodeId,
            ast: &mut Ast,
            _ctx: &mut Context<'_>,
            _matched: &Matched<'_>,
        ) -> Vec<NodeId> {
            let (name, params, raw) = {
                let call = ast.call(node).unwrap();
                (
                    call.name.clone(),
                    call.params.clone(),
                    call.raw_body.clone(),
                )
            };

            let instance = ast.new_instance(name, params);
            let text = ast.new_text(raw);
            ast.append(instance, text);
            vec![instance]
        }
    }

    /// 展开成两个兄弟节点。
    struct SplitHandler;

    impl Handler for SplitHandler {
        fn expand_call(
            &self,
            node: NodeId,
            ast: &mut Ast,
            _ctx: &mut Context<'_>,
            _matched: &Matched<'_>,
        ) -> Vec<NodeId> {
            let (name, raw) = {
                let call = ast.call(node).unwrap();
                (call.name.clone(), call.raw_body.clone())
            };

            let head = ast.new_text(format!("[{name}]"));
            let body = ast.new_text(raw);
            vec![head, body]
        }
    }

    /// 展开器自己判定无法展开。
    struct PickyHandler;

    impl Handler for PickyHandler {
        fn expand_call(
            &self,
            node: NodeId,
            ast: &mut Ast,
            _ctx: &mut Context<'_>,
            _matched: &Matched<'_>,
        ) -> Vec<NodeId> {
            let call = ast.call(node).unwrap();
            let (span, raw) = (call.span, call.raw_body.clone());

            vec![ast.new_error(ErrorNode::new(
                ErrorKind::ExpandFailed,
                "参数不全",
                span,
                raw,
            ))]
        }
    }

    fn run(source: &str, registry: Registry) -> Ast {
        let mut ast = parse(source);
        let mut ctx = Context::new(source);
        Dispatcher::new(registry).run(&mut ast, &mut ctx);
        assert!(ast.validate(), "展开后树结构应当自洽");
        ast
    }

    #[test]
    fn a_registered_natural_handler_is_used() {
        let mut registry = Registry::new();
        registry.register_natural(ParagraphHandler);

        let ast = run("正文\n", registry);

        assert_eq!(sexpr(&ast), r#"(paragraph text("正文"))"#);
    }

    #[test]
    fn an_unhandled_natural_block_falls_back_with_its_content() {
        // 自然块没有超然待遇：没注册自然块展开器时，它和未知调用块一样走兜底。
        let source = "正文\n";
        let ast = run(source, Registry::new());

        let root = ast.children(ast.document()).next().unwrap();
        let error = ast.error(root).unwrap();
        assert_eq!(error.kind, ErrorKind::NoHandler);
        assert_eq!(error.content, "正文");
        assert_eq!(error.span.slice(source), "正文");
        assert_eq!(sexpr(&ast), r#"error("没有注册自然块展开器")"#);
    }

    #[test]
    fn an_unhandled_call_block_falls_back_with_its_content() {
        let source = "::nope a=1:\n  body\n";
        let ast = run(source, Registry::new());

        let root = ast.children(ast.document()).next().unwrap();
        let error = ast.error(root).unwrap();
        assert_eq!(error.kind, ErrorKind::NoHandler);
        assert!(error.message.contains("::nope"));
        // 兜底展开器把整块原文塞进了报错节点，块体内容没有丢。
        assert_eq!(error.content, "::nope a=1:\n  body");
        assert_eq!(error.span, Span::new(1, 2, 0, source.trim_end().len()));
    }

    #[test]
    fn natural_and_call_blocks_are_dispatched_independently() {
        let mut registry = Registry::new();
        registry.register("notice", NoticeHandler);
        registry.register_natural(ParagraphHandler);

        let ast = run("正文\n\n::notice type=warning:\n  小心\n", registry);

        assert_eq!(
            sexpr(&ast),
            r#"(paragraph text("正文")) (instance notice type=warning (paragraph text("小心")))"#
        );
    }

    #[test]
    fn the_fallback_can_be_replaced() {
        let mut registry = Registry::new();
        registry.set_fallback(NoticeHandler);

        let ast = run("::nope:\n", registry);

        assert_eq!(sexpr(&ast), "instance nope");
    }

    #[test]
    fn handing_over_the_body_keeps_expanding_the_subtree() {
        let mut registry = Registry::new();
        registry.register("notice", NoticeHandler);
        registry.register_natural(ParagraphHandler);

        let ast = run("::notice type=warning:\n  ::notice:\n    内层\n", registry);

        assert_eq!(
            sexpr(&ast),
            r#"(instance notice type=warning (instance notice (paragraph text("内层"))))"#
        );
    }

    #[test]
    fn a_verbatim_handler_swallows_its_children() {
        // 这一条是整个架构的关键：::code 里的 ::notice 是**字面文本**，
        // 既不该被展开，也不该被当成未知块报错。
        let mut registry = Registry::new();
        registry.register("notice", NoticeHandler);
        registry.register("code", VerbatimHandler);
        registry.register_natural(ParagraphHandler);

        let ast = run(
            "::code lang=neomark:\n  ::notice type=warning:\n    小心\n",
            registry,
        );

        assert_eq!(
            sexpr(&ast),
            r#"(instance code lang=neomark text("::notice type=warning:\n  小心"))"#
        );
    }

    #[test]
    fn a_handler_may_expand_into_several_siblings() {
        let mut registry = Registry::new();
        registry.register("twice", SplitHandler);

        let ast = run("::twice:\n  hi\n", registry);

        assert_eq!(sexpr(&ast), r#"text("[twice]") text("hi")"#);
    }

    #[test]
    fn a_handler_may_emit_raw_elements_instead_of_semantic_nodes() {
        // 逃生口：外部展开器可以自己写 HTML 元素，语义节点不是唯一选项。
        let mut registry = Registry::new();
        registry.register("tip", RawElementHandler);
        registry.register_natural(ParagraphHandler);

        let ast = run("::tip:\n  正文\n", registry);

        assert_eq!(
            sexpr(&ast),
            r#"(element aside class=tip (paragraph text("正文")))"#
        );
    }

    #[test]
    fn an_inline_call_without_an_expander_falls_back_like_a_block_call() {
        // 行内调用与块调用一样：没人认领就交给兜底展开器（默认报错）。
        // 行内图标 `:name:` 才是不进调度器、由渲染器查表的那一类。
        let mut registry = Registry::new();
        registry.register_natural(RealNaturalExpander::default());

        let ast = run("正文 {{nope}} 结束\n", registry);

        assert_eq!(
            sexpr(&ast),
            r#"(paragraph text("正文 ") error("没有展开器能处理行内调用 {nope}") text(" 结束"))"#
        );
    }

    #[test]
    fn a_registered_inline_expander_takes_over() {
        let mut registry = Registry::new();
        registry.register_natural(RealNaturalExpander::default());
        registry.register("badge", BadgeHandler);

        let ast = run("{{badge level=3: **新**}}\n", registry);

        // 内容里的行内标记照常生效，而且**没有**多套一层段落
        assert_eq!(
            sexpr(&ast),
            r#"(paragraph (instance badge level=3 (strong text("新"))))"#
        );
    }

    #[test]
    fn a_registered_handler_that_does_not_handle_inline_calls_fails_loudly() {
        // NoticeHandler 只实现了 expand_call；同名行内调用走默认实现 → 报错
        let mut registry = Registry::new();
        registry.register_natural(RealNaturalExpander::default());
        registry.register("notice", NoticeHandler);

        let ast = run("{{notice}}\n", registry);

        assert_eq!(
            sexpr(&ast),
            r#"(paragraph error("没有展开器能处理行内调用 {notice}"))"#
        );
    }

    #[test]
    fn exact_names_beat_patterns_and_later_patterns_beat_earlier_ones() {
        let build = || {
            let mut registry = Registry::new();
            registry.register(regex::Regex::new("^h.*$").unwrap(), Marker("star"));
            registry.register(regex::Regex::new("^h.$").unwrap(), Marker("question"));
            registry.register("h1", Marker("exact"));
            registry
        };

        // 精确名最优先
        assert_eq!(sexpr(&run("::h1:\n", build())), r#"text("exact:h1")"#);
        // 两条通配都命中时，后注册的赢
        assert_eq!(sexpr(&run("::h2:\n", build())), r#"text("question:h2")"#);
        // 只有前一条通配命中
        assert_eq!(sexpr(&run("::hello:\n", build())), r#"text("star:hello")"#);
        // 都不命中 → 兜底报错
        assert_eq!(
            sexpr(&run("::nope:\n", build())),
            r#"error("未注册的调用块 ::nope")"#
        );
    }

    #[test]
    fn a_pattern_registered_twice_keeps_only_the_last_handler() {
        let mut registry = Registry::new();
        registry.register(regex::Regex::new("^h.$").unwrap(), Marker("first"));
        registry.register(regex::Regex::new("^h.$").unwrap(), Marker("second"));

        assert_eq!(registry.patterns().count(), 1);
        assert_eq!(sexpr(&run("::h3:\n", registry)), r#"text("second:h3")"#);
    }

    #[test]
    fn a_handler_may_fail_on_its_own() {
        let mut registry = Registry::new();
        registry.register("picky", PickyHandler);

        let ast = run("::picky:\n", registry);

        assert_eq!(sexpr(&ast), r#"error("参数不全")"#);
    }
}
