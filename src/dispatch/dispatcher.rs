//! 调度器：把未展开的块推进到不动点。

use super::handler::{Context, Handler};
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
        if !ast.is_unparsed(node) {
            // 已展开的节点：继续往下走。
            let children: Vec<NodeId> = ast.children(node).collect();
            for child in children {
                self.expand_node(ast, child, ctx);
            }
            return;
        }

        let handler = self.handler_for(ast, node);
        let replacement = handler.expand(node, ast, ctx);

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

    /// 为未展开节点挑展开器：没命中就用兜底。
    fn handler_for<'a>(&'a self, ast: &Ast, node: NodeId) -> &'a dyn Handler {
        match ast.tag(node) {
            Some(KindTag::Call) => ast
                .call(node)
                .and_then(|call| self.registry.get(&call.name))
                .unwrap_or_else(|| self.registry.fallback()),
            Some(KindTag::Natural) => self
                .registry
                .natural()
                .unwrap_or_else(|| self.registry.fallback()),
            _ => self.registry.fallback(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_util::sexpr;
    use crate::ast::{ErrorKind, ErrorNode, Span};
    use crate::parse::parse;

    /// 只关心调用块：把调用**换成一个模板实例**，并把原有子节点重新挂载过去。
    ///
    /// 注意它不碰任何 HTML——产出什么标签、什么类名是渲染器的事。
    struct NoticeHandler;

    impl Handler for NoticeHandler {
        fn expand_call(&self, node: NodeId, ast: &mut Ast, _ctx: &mut Context<'_>) -> Vec<NodeId> {
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
        fn expand_call(&self, node: NodeId, ast: &mut Ast, _ctx: &mut Context<'_>) -> Vec<NodeId> {
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
        fn expand_call(&self, node: NodeId, ast: &mut Ast, _ctx: &mut Context<'_>) -> Vec<NodeId> {
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
        fn expand_call(&self, node: NodeId, ast: &mut Ast, _ctx: &mut Context<'_>) -> Vec<NodeId> {
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
    fn a_handler_may_fail_on_its_own() {
        let mut registry = Registry::new();
        registry.register("picky", PickyHandler);

        let ast = run("::picky:\n", registry);

        assert_eq!(sexpr(&ast), r#"error("参数不全")"#);
    }
}
