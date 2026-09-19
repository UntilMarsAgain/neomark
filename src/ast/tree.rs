//! 一棵保存在 `indextree::Arena` 里的块树。
//!
//! # 默认是语义，`Element` 是逃生口
//!
//! [`NodeKind`] 的**默认**表达方式是语义：这是段落、这是强调、这是一个
//! `notice` 模板实例。标签名、类名、void 元素全部是 [`crate::html`] 的事，
//! 所以改 `<em>` 的类名、把 emoji 换成手搓 SVG、给 `notice` 换个标签，
//! 都只动渲染器。
//!
//! 但调用块展开器——**尤其是外部传进来的那些**——不一定能把自己塞进这套
//! 标准语义里。所以保留 [`NodeKind::Element`]：一个带标签与属性的通用节点，
//! 渲染器按 HTML 元素原样输出。它是**逃生口**，不是默认写法；能用语义节点
//! 表达的就该用语义节点。
//!
//! # 孩子在哪里
//!
//! 孩子不在节点身上，而由 arena 的父子边表示。`Unparsed` 的块体、
//! 段落的行内内容、强调包住的内容，都是 arena 边。

use indextree::{Arena, NodeId};

use super::block::{Block, CallBlock, NaturalBlock};
use super::error::ErrorNode;
use super::params::Params;

/// 节点载荷：**语义**种类，不含任何 HTML。
///
/// 除文档根外只有两根轴：**解析了没有**，以及**它是什么意思**。
#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    /// 文档根：整棵树的唯一根。不参与展开，遍历从它的子节点开始。
    Document,
    /// **未解析**的块；具体是哪一种语法块由 [`Block`] 说明。
    Unparsed(Block),

    // ── 块级语义 ──────────────────────────────────────────
    /// 段落：孩子是行内节点。
    Paragraph,
    /// 调用块展开出的**模板实例**。
    ///
    /// 只记名字与参数——它长什么样完全由渲染器决定。这与 [`NodeKind::Emoji`]
    /// 是同一种做法：AST 负责「这是什么」，渲染器负责「长什么样」。
    Instance {
        /// 模板名，例如 `notice`。
        name: String,
        /// 调用时给的参数。
        params: Params,
    },

    // ── 行内语义 ──────────────────────────────────────────
    /// 强调。
    Emphasis,
    /// 加粗。
    Strong,
    /// 删除线。
    Strikethrough,
    /// 下标。
    Subscript,
    /// 上标。
    Superscript,
    /// 高亮。
    Mark,
    /// 行内代码；孩子是一个**原样**文本。
    Code,
    /// 行内数学；孩子是一个**原样**文本。
    Math,
    /// Emoji 短码；只记名字，怎么显示由渲染器决定。
    Emoji(String),
    /// 硬换行。
    LineBreak,

    // ── 逃生口 ───────────────────────────────────────────
    /// 通用 HTML 元素：标签 + 属性，孩子是它的子节点。
    ///
    /// **逃生口，不是默认写法。** 给那些无法用上面的语义节点表达的调用块
    /// 展开器用——外部传进来的插件尤其如此。渲染器把它当 HTML 元素原样
    /// 输出，所以往这里写的东西，等于绕过了 `crate::html` 的映射。
    ///
    /// 能用 [`NodeKind::Paragraph`]、[`NodeKind::Emphasis`]、
    /// [`NodeKind::Instance`] 表达的，就不要用这个。
    Element {
        /// 标签名，例如 `img`、`pre`、`circle`。
        tag: String,
        /// 属性。
        attrs: Vec<Attr>,
    },

    // ── 内容 ─────────────────────────────────────────────
    /// 纯文本（**未转义**；转义是渲染器的事）。
    Text(String),
    /// 报错节点：展开器无法工作时的**最终回退**，是叶子。
    Error(ErrorNode),
}

/// 「这个节点该怎么处理」的标签，**不是**载荷形状的镜像。
///
/// 它刻意保持很小：调度器只需要知道「有没有解析」「按名字查还是走自然块
/// 槽位」，其余全是已经展开的语义节点。`Copy` 是为了能在**不持有 `&Ast`**
/// 的情况下做判断——拿 `&NodeKind` 的同时又需要 `&mut Ast` 会被借用检查拒绝。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KindTag {
    /// 文档根。
    Document,
    /// 未解析的自然块：没有名字，走专门槽位。
    Natural,
    /// 未解析的调用块：按名字查找。
    Call,
    /// 已经展开的语义节点。
    Expanded,
}

impl NodeKind {
    /// 「该怎么处理」的标签。
    pub const fn tag(&self) -> KindTag {
        match self {
            NodeKind::Document => KindTag::Document,
            NodeKind::Unparsed(Block::Natural(_)) => KindTag::Natural,
            NodeKind::Unparsed(Block::Call(_)) => KindTag::Call,
            _ => KindTag::Expanded,
        }
    }

    /// 是否是尚未解析的块。
    pub const fn is_unparsed(&self) -> bool {
        matches!(self, NodeKind::Unparsed(_))
    }
}

/// 一棵块树。
#[derive(Debug, Clone)]
pub struct Ast {
    arena: Arena<NodeKind>,
    document: NodeId,
}

impl Default for Ast {
    fn default() -> Self {
        Self::new()
    }
}

impl Ast {
    /// 新建一棵只有文档根的树。
    pub fn new() -> Self {
        let mut arena = Arena::new();
        let document = arena.new_node(NodeKind::Document);
        Self { arena, document }
    }

    /// 文档根。整棵树的遍历都从它的子节点开始。
    pub fn document(&self) -> NodeId {
        self.document
    }

    /// 底层的 arena。展开器需要兄弟导航、重新挂载、删除子树等操作时用它。
    pub fn arena(&self) -> &Arena<NodeKind> {
        &self.arena
    }

    /// 底层的 arena（可变）。
    pub fn arena_mut(&mut self) -> &mut Arena<NodeKind> {
        &mut self.arena
    }

    /// 树结构自检（用于测试）。
    pub fn validate(&self) -> bool {
        self.arena.validate()
    }

    // ── 读取 ───────────────────────────────────────────────

    /// 节点的语义载荷。
    pub fn kind(&self, id: NodeId) -> Option<&NodeKind> {
        self.arena.get_data(id)
    }

    /// 节点的语义载荷（可变）。
    pub fn kind_mut(&mut self, id: NodeId) -> Option<&mut NodeKind> {
        self.arena.get_data_mut(id)
    }

    /// 「该怎么处理」的标签。
    pub fn tag(&self, id: NodeId) -> Option<KindTag> {
        self.arena.get_data(id).map(NodeKind::tag)
    }

    /// 节点是否是尚未解析的块。
    pub fn is_unparsed(&self, id: NodeId) -> bool {
        self.arena.get_data(id).is_some_and(NodeKind::is_unparsed)
    }

    /// 未解析块的语法载荷。
    pub fn block(&self, id: NodeId) -> Option<&Block> {
        match self.arena.get_data(id)? {
            NodeKind::Unparsed(block) => Some(block),
            _ => None,
        }
    }

    /// 自然块的载荷。
    pub fn natural(&self, id: NodeId) -> Option<&NaturalBlock> {
        match self.block(id)? {
            Block::Natural(natural) => Some(natural),
            Block::Call(_) => None,
        }
    }

    /// 调用块的载荷。
    pub fn call(&self, id: NodeId) -> Option<&CallBlock> {
        match self.block(id)? {
            Block::Call(call) => Some(call),
            Block::Natural(_) => None,
        }
    }

    /// 纯文本。
    pub fn text(&self, id: NodeId) -> Option<&str> {
        match self.arena.get_data(id)? {
            NodeKind::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Emoji 短码名。
    pub fn emoji(&self, id: NodeId) -> Option<&str> {
        match self.arena.get_data(id)? {
            NodeKind::Emoji(alias) => Some(alias),
            _ => None,
        }
    }

    /// 模板实例的名字与参数。
    pub fn instance(&self, id: NodeId) -> Option<(&str, &Params)> {
        match self.arena.get_data(id)? {
            NodeKind::Instance { name, params } => Some((name, params)),
            _ => None,
        }
    }

    /// 通用元素的标签与属性。
    pub fn element(&self, id: NodeId) -> Option<(&str, &[Attr])> {
        match self.arena.get_data(id)? {
            NodeKind::Element { tag, attrs } => Some((tag, attrs)),
            _ => None,
        }
    }

    /// 报错节点。
    pub fn error(&self, id: NodeId) -> Option<&ErrorNode> {
        match self.arena.get_data(id)? {
            NodeKind::Error(error) => Some(error),
            _ => None,
        }
    }

    // ── 建立 ───────────────────────────────────────────────

    /// 新建一个任意语义载荷的节点（**游离**状态，尚未挂进树）。
    pub fn new_node(&mut self, kind: NodeKind) -> NodeId {
        self.arena.new_node(kind)
    }

    /// 新建一个文本节点。
    pub fn new_text(&mut self, text: impl Into<String>) -> NodeId {
        self.new_node(NodeKind::Text(text.into()))
    }

    /// 新建一个段落节点。
    pub fn new_paragraph(&mut self) -> NodeId {
        self.new_node(NodeKind::Paragraph)
    }

    /// 新建一个模板实例节点。
    pub fn new_instance(&mut self, name: impl Into<String>, params: Params) -> NodeId {
        self.new_node(NodeKind::Instance {
            name: name.into(),
            params,
        })
    }

    /// 新建一个通用元素节点（**逃生口**，见 [`NodeKind::Element`]）。
    pub fn new_element(&mut self, tag: impl Into<String>, attrs: Vec<Attr>) -> NodeId {
        self.new_node(NodeKind::Element {
            tag: tag.into(),
            attrs,
        })
    }

    /// 新建一个 Emoji 节点。
    pub fn new_emoji(&mut self, alias: impl Into<String>) -> NodeId {
        self.new_node(NodeKind::Emoji(alias.into()))
    }

    /// 新建一个硬换行节点。
    pub fn new_line_break(&mut self) -> NodeId {
        self.new_node(NodeKind::LineBreak)
    }

    /// 新建一个报错节点。
    pub fn new_error(&mut self, error: ErrorNode) -> NodeId {
        self.new_node(NodeKind::Error(error))
    }

    // ── 结构操作 ───────────────────────────────────────────

    /// 把 `child` 挂为 `parent` 的最后一个孩子。
    ///
    /// 如果 `child` 本来就挂在别处，`indextree` 会先把它摘下来——所以
    /// 「把块体移交给新元素」就是一次 `append`。
    pub fn append(&mut self, parent: NodeId, child: NodeId) {
        parent.append(child, &mut self.arena);
    }

    /// 把 `node` 插到 `anchor` 之后（成为其下一个兄弟）。
    pub fn insert_after(&mut self, anchor: NodeId, node: NodeId) {
        anchor.insert_after(node, &mut self.arena);
    }

    /// 删除 `node` 及其整棵子树。
    pub fn remove_subtree(&mut self, node: NodeId) {
        node.remove_subtree(&mut self.arena);
    }

    /// 遍历 `parent` 的孩子（文档顺序）。
    pub fn children(&self, parent: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        parent.children(&self.arena)
    }

    /// 父节点。
    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        id.parent(&self.arena)
    }

    /// 把一个块挂到文档根下。
    pub fn push_block(&mut self, id: NodeId) {
        self.append(self.document, id);
    }
}

/// 通用元素的属性。
///
/// 只在 [`NodeKind::Element`] 这个逃生口里出现——上面的语义节点不携带属性，
/// 它们长什么样是 [`crate::html`] 的事。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    /// 属性名。
    pub name: String,
    /// 属性值；`None` 表示**布尔属性**（`lazy` 而不是 `lazy="true"`）。
    pub value: Option<String>,
}

impl Attr {
    /// 普通属性。
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: Some(value.into()),
        }
    }

    /// 布尔属性。
    pub fn boolean(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: None,
        }
    }

    /// 是否是布尔属性。
    pub const fn is_boolean(&self) -> bool {
        self.value.is_none()
    }
}
