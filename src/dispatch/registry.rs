//! 展开器的运行期注册表。

use std::collections::HashMap;

use regex::Regex;

use super::handler::{Fallback, Handler};
use super::key::Key;

/// 一次按名查找的结果：展开器 + [命中信息](crate::dispatch::Matched)。
///
/// 两个生命周期分别是注册表的借用（`'r`）与调用名的借用（`'h`）——`Captures`
/// 借用调用名，而展开器借用注册表，两者互不相干。
pub struct Found<'r, 'h> {
    /// 命中的展开器。
    pub handler: &'r dyn Handler,
    /// 正则命中信息；精确名命中时为 `None`。
    pub captures: Option<regex::Captures<'h>>,
}

/// 一条**正则模式**注册。
struct Pattern {
    regex: Regex,
    handler: Box<dyn Handler>,
}

/// 展开器注册表。
///
/// * 调用块按**调用名**查找：先精确匹配，再按**正则模式**匹配；
/// * 自然块查找专门的**自然块展开器**；
/// * 都没命中时用**兜底展开器**，它默认把块的原文放进报错节点。
///
/// 同名重复注册会覆盖旧的展开器（正则模式也一样：后注册的优先）。
///
/// # 正则模式
///
/// 模式就是**普通的正则**，没有自造语法：`h[1-6]`、`note-(info|warning)`、
/// `(?i)badge-\w+` 都照正则理解。
///
/// 模式在注册时**已经编译好**（参数是 [`Regex`]），所以「模式写错」在写模式的
/// 地方就处理掉了，注册表内部不会出现编译失败。
///
/// 调用名是**整名**，所以通常要自己加锚点：`^h[1-6]$` 恰好匹配 `h1`~`h6`，
/// 而 `h[1-6]`（无锚点）还会命中 `xh1y`。是否锚定由模式自己说了算，注册表
/// 不代替调用者决定。
///
/// 查找优先级：**精确名 > 后注册的模式 > 先注册的模式 > 兜底**。展开器想知道
/// 自己匹配到了哪个名字，读 `ast.call(node).name` 即可。
pub struct Registry {
    handlers: HashMap<String, Box<dyn Handler>>,
    patterns: Vec<Pattern>,
    natural: Option<Box<dyn Handler>>,
    fallback: Box<dyn Handler>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            handlers: HashMap::new(),
            patterns: Vec::new(),
            natural: None,
            fallback: Box::new(Fallback),
        }
    }
}

impl Registry {
    /// 空注册表；兜底展开器为 [`Fallback`]。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册（或覆盖）一个展开器：**键写什么就怎么匹配**。
    ///
    /// * `&str` / `String` → 精确调用名（查表，优先级最高）；
    /// * `Regex` → 正则模式（逐条试，后注册的优先）。
    ///
    /// 见 [`Key`]。同一个键重复注册会覆盖旧的。
    ///
    /// ```
    /// use neomark::{Registry, handlers, regex::Regex};
    ///
    /// let mut registry = Registry::new();
    ///
    /// registry.register("notice", handlers::Wrap::tag("div").class("nm-notice"));
    /// registry.register(
    ///     Regex::new("^h[1-6]$").unwrap(),
    ///     handlers::Wrap::tag_from_match().class_from_match(),
    /// );
    /// ```
    pub fn register(&mut self, key: impl Into<Key>, handler: impl Handler + 'static) -> &mut Self {
        let handler = Box::new(handler);

        match key.into() {
            Key::Name(name) => {
                self.handlers.insert(name, handler);
            }
            Key::Pattern(regex) => match self
                .patterns
                .iter_mut()
                .find(|entry| entry.regex.as_str() == regex.as_str())
            {
                Some(entry) => entry.handler = handler,
                None => self.patterns.push(Pattern { regex, handler }),
            },
        }

        self
    }

    /// 注册**自然块**展开器。
    pub fn register_natural(&mut self, handler: impl Handler + 'static) -> &mut Self {
        self.natural = Some(Box::new(handler));
        self
    }

    /// 替换**兜底**展开器。
    pub fn set_fallback(&mut self, handler: impl Handler + 'static) -> &mut Self {
        self.fallback = Box::new(handler);
        self
    }

    /// 按调用名查找展开器：精确优先，其次正则（后注册的优先）。
    pub fn get(&self, name: &str) -> Option<&dyn Handler> {
        self.find(name).map(|found| found.handler)
    }

    /// 按调用名查找展开器，**连同正则的命中信息**。
    ///
    /// [`Registry::get`] 只是它的简化形式。需要捕获组的展开器要用这个：
    /// 返回的 `Captures` 借用 `name`，所以调用方通常得先把名字复制成局部变量，
    /// 这样它和 `&mut Ast` 才不冲突。
    pub fn find<'r, 'h>(&'r self, name: &'h str) -> Option<Found<'r, 'h>> {
        if let Some(handler) = self.handlers.get(name) {
            return Some(Found {
                handler: handler.as_ref(),
                captures: None,
            });
        }

        self.patterns.iter().rev().find_map(|entry| {
            entry.regex.captures(name).map(|captures| Found {
                handler: entry.handler.as_ref(),
                captures: Some(captures),
            })
        })
    }

    /// 自然块展开器；没注册过时为 `None`。
    pub fn natural(&self) -> Option<&dyn Handler> {
        self.natural.as_deref()
    }

    /// 兜底展开器。
    pub fn fallback(&self) -> &dyn Handler {
        self.fallback.as_ref()
    }

    /// 是否注册过这个调用名（精确或正则命中）。
    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// 已注册的精确调用名，顺序不定。
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.handlers.keys().map(String::as_str)
    }

    /// 已注册的正则模式原文，按注册顺序。
    pub fn patterns(&self) -> impl Iterator<Item = &str> {
        self.patterns.iter().map(|entry| entry.regex.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(pattern: &str, name: &str) -> bool {
        Regex::new(pattern)
            .expect("测试用的模式都应当是合法的")
            .is_match(name)
    }

    #[test]
    fn the_default_heading_pattern_is_anchored_and_covers_h1_to_h6() {
        let pattern = crate::handlers::HEADING_PATTERN;

        for level in 1..=6 {
            assert!(
                m(pattern, &format!("h{level}")),
                "{pattern} 应当匹配 h{level}"
            );
        }
        for name in ["h0", "h7", "h10", "h", "ha", "xh1", "h1y", "H1"] {
            assert!(!m(pattern, name), "{pattern} 不应当匹配 {name}");
        }
    }

    #[test]
    fn patterns_are_plain_regexes() {
        assert!(m("^note-(info|warning)$", "note-warning"));
        assert!(!m("^note-(info|warning)$", "note-error"));
        assert!(m("^badge-\\w+$", "badge-new"));
        assert!(m("^(?i)h[1-6]$", "H3"), "行内标志照常可用");
        // 无锚点就是部分匹配——这正是正则的语义，注册表不替调用者加锚
        assert!(m("h[1-6]", "xh1y"));
    }
}
