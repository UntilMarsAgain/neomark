//! 展开器的运行期注册表。

use std::collections::HashMap;

use super::handler::{Fallback, Handler};

/// 一条**通配**注册。
struct Pattern {
    pattern: String,
    handler: Box<dyn Handler>,
}

/// 展开器注册表。
///
/// * 调用块按**调用名**查找：先精确匹配，再按**通配模式**匹配；
/// * 自然块查找专门的**自然块展开器**；
/// * 都没命中时用**兜底展开器**，它默认把块的原文放进报错节点。
///
/// 同名重复注册会覆盖旧的展开器（通配模式也一样：后注册的优先）。
///
/// # 通配
///
/// 模式里只有两个元字符：`*` 匹配任意（含空）字符序列，`?` 匹配恰好一个字符。
/// 不引入正则引擎——`h?`（`h1`~`h9`）、`note-*` 这类族名够用了。
///
/// 查找优先级：**精确名 > 后注册的通配模式 > 先注册的通配模式 > 兜底**。
/// 展开器想知道自己匹配到了哪个名字，读 `ast.call(node).name` 即可。
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

    /// 注册（或覆盖）一个**调用名**的展开器。
    pub fn register(
        &mut self,
        name: impl Into<String>,
        handler: impl Handler + 'static,
    ) -> &mut Self {
        self.handlers.insert(name.into(), Box::new(handler));
        self
    }

    /// 注册一个**通配模式**的展开器。
    ///
    /// 模式里 `*` 匹配任意（含空）字符序列，`?` 匹配恰好一个字符。同一个模式
    /// 重复注册会覆盖旧的；不同模式之间**后注册的优先**。
    pub fn register_pattern(
        &mut self,
        pattern: impl Into<String>,
        handler: impl Handler + 'static,
    ) -> &mut Self {
        let pattern = pattern.into();
        let handler = Box::new(handler);

        match self
            .patterns
            .iter_mut()
            .find(|entry| entry.pattern == pattern)
        {
            Some(entry) => entry.handler = handler,
            None => self.patterns.push(Pattern { pattern, handler }),
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

    /// 按调用名查找展开器：精确优先，其次通配（后注册的优先）。
    pub fn get(&self, name: &str) -> Option<&dyn Handler> {
        if let Some(handler) = self.handlers.get(name) {
            return Some(handler.as_ref());
        }

        self.patterns
            .iter()
            .rev()
            .find(|entry| matches(&entry.pattern, name))
            .map(|entry| entry.handler.as_ref())
    }

    /// 自然块展开器；没注册过时为 `None`。
    pub fn natural(&self) -> Option<&dyn Handler> {
        self.natural.as_deref()
    }

    /// 兜底展开器。
    pub fn fallback(&self) -> &dyn Handler {
        self.fallback.as_ref()
    }

    /// 是否注册过这个调用名（精确或通配命中）。
    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// 已注册的**精确**调用名个数（不含通配、自然块与兜底）。
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// 是否没有任何精确注册的调用名。
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    /// 已注册的精确调用名，顺序不定。
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.handlers.keys().map(String::as_str)
    }

    /// 已注册的通配模式，按注册顺序。
    pub fn patterns(&self) -> impl Iterator<Item = &str> {
        self.patterns.iter().map(|entry| entry.pattern.as_str())
    }
}

/// 极简通配匹配：`*` 匹配任意（含空）字符序列，`?` 匹配恰好一个字符。
///
/// 用经典的「记下上一个 `*` 的位置，失配就回溯」做法，不需要正则引擎。
fn matches(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();

    let (mut p, mut n) = (0, 0);
    // 上一个 `*` 之后的位置，以及它当时对齐到的名字位置。
    let mut star: Option<(usize, usize)> = None;

    while n < name.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some((p + 1, n));
            p += 1;
        } else if let Some((after_star, at)) = star {
            // 回溯：让 `*` 多吃一个字符。
            p = after_star;
            n = at + 1;
            star = Some((after_star, at + 1));
        } else {
            return false;
        }
    }

    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }

    p == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matches_literals_question_marks_and_stars() {
        assert!(matches("h?", "h1"));
        assert!(matches("h?", "h9"));
        assert!(!matches("h?", "h10"));
        assert!(!matches("h?", "h"));

        assert!(matches("note-*", "note-warning"));
        assert!(matches("note-*", "note-"));
        assert!(!matches("note-*", "notice"));

        assert!(matches("*", "anything"));
        assert!(matches("*", ""));
        assert!(matches("a*b*c", "aXXbYYc"));
        assert!(!matches("a*b*c", "aXXbYY"));
        assert!(matches("h*", "hello"), "`*` 是任意序列，不限于数字");
    }

    #[test]
    fn unmatched_survivors_are_not_accepted() {
        assert!(!matches("abc", "ab"));
        assert!(!matches("ab", "abc"));
        assert!(matches("a**b", "ab"));
    }
}
