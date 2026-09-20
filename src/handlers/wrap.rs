//! 包装器展开器：把调用换成一层元素。
//!
//! 大多数展开器其实只干一件事——「外面套一层别的标签，或者一个带 class 的
//! `div`」。所以这里给注册这一步配个语法糖。

use crate::ast::{Ast, Attr, NodeId};
use crate::dispatch::{Context, Handler, Invocation, Matched};

use super::natural::NaturalExpander;

/// 由[调用信息](Invocation)算出一个属性值，例如「类名 = language 参数」。
type Derive = Box<dyn Fn(&Invocation<'_>) -> String>;

/// 同上，但可以表示「这个属性不该出现」。
type AttrDerive = Box<dyn Fn(&Invocation<'_>) -> Option<String>>;

/// 通用包装器：把调用整个换成一层元素。
///
/// 标签名与类名既可以写死，也可以**由命中信息算出来**——后者让一条注册覆盖一族
/// 调用名：
///
/// ```
/// use neomark::{Invocation, Registry, handlers::Wrap};
/// use regex::Regex;
///
/// let mut registry = Registry::new();
///
/// // 写死：`::notice` → <div class="nm-notice">…</div>
/// registry.register("notice", Wrap::tag("div").class("nm-notice"));
///
/// // 算出来：`::h3` → <h3 class="nm-h3">正文</h3>
/// registry.register(
///     Regex::new("^h[1-6]$").unwrap(),
///     Wrap::tag_from(|call: &Invocation| call.matched().to_string())
///         .class_from(|call: &Invocation| format!("nm-{}", call.matched())),
/// );
/// ```
///
/// 用捕获组也行：`m.capture(1)`、`m.capture_named("kind")` 都取得到。
///
/// # 两种包装
///
/// * **块级包装**（默认）：块体的块子树整体搬进元素，所以内层照常展开——
///   `::box:` 里套 `::h2` 会得到 `<section><h2>…</h2></section>`。
/// * **行内包装**（[`Wrap::inline`]）：块体文本按**行内层**解析，不套段落。
///   `<h3>` 里不能有 `<p>`，所以标题必须走这条。
///
/// # 它和内置渲染器的关系
///
/// 它产出的是 [`NodeKind::Element`](crate::NodeKind::Element)，也就是那个
/// **逃生口**：直接写 HTML 标签，不走渲染器的语义映射，也不受 `nm-` 命名约定
/// 约束。想让自家那一族类名与内置输出风格一致，自己给类名加前缀即可。
///
/// 只处理**调用块**。自然块与行内调用走 [`Handler`] 的默认实现（报错节点），
/// 因为「包装一个自然块」的意思不明确——是包住每一段，还是包住整块文本。
pub struct Wrap {
    tag: String,
    tag_from: Option<Derive>,
    classes: Vec<String>,
    classes_from: Vec<Derive>,
    attrs: Vec<Attr>,
    attrs_from: Vec<(String, AttrDerive)>,
    class_prefix: String,
    inline: Option<NaturalExpander>,
}

impl Wrap {
    /// 用固定标签构造。
    pub fn tag(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            ..Self::empty()
        }
    }

    /// 标签名由[调用信息](Invocation)算出来。
    ///
    /// `Wrap::tag_from(|call: &Invocation| call.matched().to_string())` 让标签等于
    /// 正则实际匹配到的那一段——`^h[1-6]$` 命中 `h3` 就产出 `<h3>`。
    ///
    /// 与 [`tag`](Wrap::tag) 二选一：标签要么写死，要么算出来。
    ///
    /// 闭包参数**要写出类型**（`|call: &Invocation|`）：`impl Fn` 参数上的闭包
    /// 推断拿不到这个匿名生命周期，不写就编不过。
    pub fn tag_from(derive: impl Fn(&Invocation<'_>) -> String + 'static) -> Self {
        Self {
            tag_from: Some(Box::new(derive)),
            ..Self::empty()
        }
    }

    /// 空包装器，只给上面两个构造器当前缀用。
    fn empty() -> Self {
        Self {
            tag: String::new(),
            tag_from: None,
            classes: Vec::new(),
            classes_from: Vec::new(),
            attrs: Vec::new(),
            attrs_from: Vec::new(),
            class_prefix: String::new(),
            inline: None,
        }
    }

    /// 标签名取**正则实际匹配到的那一段**。
    ///
    /// 等价于 `tag_from(|call: &Invocation| call.matched().to_string())`，但不用给
    /// 闭包参数写类型。内置标题用的就是它。
    pub fn tag_from_match() -> Self {
        Self::tag_from(|call: &Invocation<'_>| call.matched().to_string())
    }

    /// 追加一个固定类名。
    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.classes.push(class.into());
        self
    }

    /// 加一个「正则**实际匹配到的那一段**」当类名（会净化）。
    ///
    /// 等价于 `class_from(|call: &Invocation| call.matched().to_string())`。
    pub fn class_from_match(self) -> Self {
        self.class_from(|call: &Invocation<'_>| sanitize_class(call.matched()))
    }

    /// 给**派生出来的**类名加统一前缀。
    ///
    /// 只影响 [`class_from_match`](Wrap::class_from_match) /
    /// [`class_from_name`](Wrap::class_from_name) / [`class_from`](Wrap::class_from)
    /// 的结果，不影响 [`class`](Wrap::class) 写死的类名。
    /// 内置标题就是 `class_prefix("nm-")` + `class_from_match()` → `nm-h3`。
    pub fn class_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.class_prefix = prefix.into();
        self
    }

    /// 追加一个由[调用信息](Invocation)算出的类名。
    ///
    /// **想用捕获组或参数就用这里**：`call.capture(1)`、`call.capture_named("kind")`、
    /// `call.param("language")` 都能取，几个拼一个类名也行：
    ///
    /// ```
    /// use neomark::{Invocation, handlers::Wrap};
    ///
    /// // `^figure-(?P<kind>\w+)-v(?P<version>\d+)$` 命中 `figure-chart-v2`
    /// let wrap = Wrap::tag("figure").class("figure").class_from(|call: &Invocation| {
    ///     format!(
    ///         "{}-v{}",
    ///         call.capture_named("kind").unwrap_or("unknown"),
    ///         call.capture_named("version").unwrap_or("0"),
    ///     )
    /// });
    /// # let _ = wrap;
    /// ```
    ///
    /// 参数与捕获组都从 [`Invocation`] 上取：`call.param("language")`、
    /// `call.capture(1)`、`call.capture_named("kind")` 都能用。
    ///
    /// 常见派生不必写闭包：[`class_from_match`](Wrap::class_from_match) /
    /// [`class_from_name`](Wrap::class_from_name) 已经覆盖；闭包参数
    /// **要写出类型**（`|call: &Invocation|`）这一步留给不常见的写法。
    pub fn class_from(mut self, derive: impl Fn(&Invocation<'_>) -> String + 'static) -> Self {
        self.classes_from.push(Box::new(derive));
        self
    }

    /// 把**完整调用名**也作为一个类名加上：`::note-warning` →
    /// `class="note note-warning"`。
    ///
    /// 是 [`class_from`](Wrap::class_from) 的常用特例。注意它用的是**调用名**而
    /// 不是正则实际匹配到的那一段：模式常常只锚开头（`^note-`），匹配到的只是
    /// 前缀，而当类名用的应当是完整的调用名。要那一段就用
    /// `class_from(|call: &Invocation| call.matched().to_string())`。
    ///
    /// 非法字符会被换成 `-`，因为类名不能带空格。
    pub fn class_from_name(self) -> Self {
        self.class_from(|call| sanitize_class(call.name()))
    }

    /// 从**参数**取一个属性：参数缺席（或为空）时不写这个属性。
    ///
    /// `::quote origin=出处` 就是 `attr_from_param("origin", "data-origin")`。
    /// 参数缺席与参数为空是两件事，所以这里对两者都不写属性——省得留下
    /// `data-origin=""` 这种噪声。
    pub fn attr_from_param(self, param: &str, attr: impl Into<String>) -> Self {
        let param = param.to_string();

        self.attr_from(attr, move |call: &Invocation<'_>| {
            call.param(&param)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
    }

    /// 追加一个由[调用信息](Invocation)算出的属性：返回 `None` 就跳过。
    ///
    /// 闭包参数**要写出类型**（`|call: &Invocation|`）。
    pub fn attr_from(
        mut self,
        attr: impl Into<String>,
        derive: impl Fn(&Invocation<'_>) -> Option<String> + 'static,
    ) -> Self {
        self.attrs_from.push((attr.into(), Box::new(derive)));
        self
    }

    /// 追加一个普通属性。
    pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.push(Attr::new(name, value));
        self
    }

    /// 追加一个布尔属性。
    pub fn flag(mut self, name: impl Into<String>) -> Self {
        self.attrs.push(Attr::boolean(name));
        self
    }

    /// 让块体按**行内**内容解析，而不是原样搬进来。
    ///
    /// 见[类型文档](Wrap#两种包装)。`<h3>` 里不能有 `<p>`，所以标题必须调它。
    pub fn inline(mut self, natural: NaturalExpander) -> Self {
        self.inline = Some(natural);
        self
    }

    /// 组装最终的标签名与属性表。
    ///
    /// **必须在动树之前算完**：派生闭包要读节点（参数就在节点里），而拿到标签与
    /// 属性之后就要 `&mut Ast` 了。
    fn build(&self, call: &Invocation<'_>) -> (String, Vec<Attr>) {
        let tag = match &self.tag_from {
            Some(derive) => derive(call),
            None => self.tag.clone(),
        };

        let mut classes: Vec<String> = self.classes.clone();
        classes.extend(
            self.classes_from
                .iter()
                .map(|derive| format!("{}{}", self.class_prefix, derive(call))),
        );
        classes.retain(|class| !class.is_empty());

        let mut attrs = self.attrs.clone();
        attrs.extend(
            self.attrs_from.iter().filter_map(|(name, derive)| {
                derive(call).map(|value| Attr::new(name.clone(), value))
            }),
        );

        if !classes.is_empty() {
            // 类名放最前面，读起来顺眼。
            attrs.insert(0, Attr::new("class", classes.join(" ")));
        }

        (tag, attrs)
    }
}

impl Handler for Wrap {
    fn expand_call(
        &self,
        node: NodeId,
        ast: &mut Ast,
        _ctx: &mut Context<'_>,
        matched: &Matched<'_>,
    ) -> Vec<NodeId> {
        // 先把标签、属性、块体都算出来（这一段要读 `Ast`），再动树。
        let (tag, attrs, span, body) = {
            let Some(call) = ast.call(node) else {
                return Vec::new();
            };
            let invocation = Invocation::new(matched, &call.params);
            let (tag, attrs) = self.build(&invocation);

            (tag, attrs, call.span, call.raw_body.clone())
        };

        let element = ast.new_element(tag, attrs);

        match &self.inline {
            // 行内包装：块体文本走行内层，块子树**不**搬移。
            Some(natural) => {
                for child in natural.inline(ast, &body, span) {
                    ast.append(element, child);
                }
            }
            // 块级包装：一次 append 就把块体搬过来了，所以内层会照常继续展开。
            None => {
                for child in ast.children(node).collect::<Vec<_>>() {
                    ast.append(element, child);
                }
            }
        }

        vec![element]
    }
}

/// 把字符串变成能当类名用的形式：只留 `[A-Za-z0-9_-]`，其余换成 `-`。
fn sanitize_class(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Params;
    use regex::Regex;

    /// 精确名命中、没有参数——最简单的一次调用。
    fn plain() -> (&'static Matched<'static>, &'static Params) {
        static EMPTY: std::sync::OnceLock<Params> = std::sync::OnceLock::new();
        static MATCHED: std::sync::OnceLock<Matched<'static>> = std::sync::OnceLock::new();

        (
            MATCHED.get_or_init(Matched::unnamed),
            EMPTY.get_or_init(Params::new),
        )
    }

    #[test]
    fn classes_are_collected_into_one_attribute_in_order() {
        let (matched, params) = plain();
        let wrap = Wrap::tag("div").class("a").class("b");
        let (tag, attrs) = wrap.build(&Invocation::new(matched, params));

        assert_eq!(tag, "div");
        assert_eq!(attrs, vec![Attr::new("class", "a b")]);
    }

    #[test]
    fn name_as_class_is_appended_last() {
        let name = "note-warning".to_string();
        let matched = Matched::new(&name, None);
        let params = Params::new();

        let wrap = Wrap::tag("div").class("note").class_from_name();
        let (_, attrs) = wrap.build(&Invocation::new(&matched, &params));

        assert_eq!(attrs, vec![Attr::new("class", "note note-warning")]);
    }

    #[test]
    fn name_as_class_is_sanitized_because_class_names_cannot_contain_spaces() {
        let (unnamed, empty) = plain();
        let wrap = Wrap::tag("div").class_from_name();

        // ASCII 部分留着，其余每个字符换成一个 `-`
        let name = "note 警告".to_string();
        let matched = Matched::new(&name, None);
        let (_, attrs) = wrap.build(&Invocation::new(&matched, &Params::new()));
        assert_eq!(attrs, vec![Attr::new("class", "note---")]);

        // 名字为空时干脆不加这个属性
        let (_, attrs) = wrap.build(&Invocation::new(unnamed, empty));
        assert!(attrs.is_empty());
    }

    #[test]
    fn tag_and_class_can_be_derived_from_the_match_or_from_captures() {
        let name = "h3".to_string();
        let captures = Regex::new("^h([1-6])$").unwrap().captures(&name);
        let matched = Matched::new(&name, captures);
        let params = Params::new();

        // 这就是要区分 `matched()` 与 `capture()` 的原因：整段匹配是 `h3`，
        // 而捕获组 1 只是 `3`。
        assert_eq!(matched.matched(), "h3");
        assert_eq!(matched.capture(1), Some("3"));

        // 标签取**整段匹配**，类名取**捕获组**
        let wrap = Wrap::tag_from(|call: &Invocation| call.matched().to_string())
            .class_from(|call: &Invocation| format!("nm-{}", call.capture(1).unwrap_or_default()));

        let (tag, attrs) = wrap.build(&Invocation::new(&matched, &params));
        assert_eq!(tag, "h3");
        assert_eq!(attrs, vec![Attr::new("class", "nm-3")]);
    }

    #[test]
    fn a_class_can_come_from_a_parameter_not_just_the_name() {
        // 补上的那件事：派生闭包读得到**参数**（`Matched` 里没有参数）。
        let wrap = Wrap::tag("code").class_from(|call: &Invocation| {
            format!("language-{}", call.param("language").unwrap_or("plaintext"))
        });

        let (matched, _) = plain();
        let mut params = Params::new();
        params.push("language", "rust");

        let (_, attrs) = wrap.build(&Invocation::new(matched, &params));
        assert_eq!(attrs, vec![Attr::new("class", "language-rust")]);
    }

    #[test]
    fn extra_attributes_survive_and_class_comes_first() {
        let (matched, params) = plain();
        let wrap = Wrap::tag("img")
            .attr("src", "a.png")
            .flag("lazy")
            .class("pic");
        let (_, attrs) = wrap.build(&Invocation::new(matched, params));

        assert_eq!(
            attrs,
            vec![
                Attr::new("class", "pic"),
                Attr::new("src", "a.png"),
                Attr::boolean("lazy"),
            ]
        );
    }
}
