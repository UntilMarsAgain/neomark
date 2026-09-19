//! 一次按名查找的**命中信息**。

use regex::Captures;

/// 这次分发的命中信息。
///
/// * **精确名**命中：没有捕获组，[`matched`](Matched::matched) 就等于调用名；
/// * **正则模式**命中：[`matched`](Matched::matched) 是正则**实际匹配到**的那
///   一段（模式没锚定时它可能短于调用名），[`capture`](Matched::capture) /
///   [`capture_named`](Matched::capture_named) 取各捕获组。
///
/// `matched()` 与 `name()` 是两回事，别混用：`^note-` 命中 `note-warning` 时
/// `matched()` 只是 `note-`，拿它当类名毫无意义；而 `^h([1-6])$` 命中 `h3` 时
/// 两者都是 `h3`。
///
/// # 生命周期
///
/// 这里的引用绑在**调用名**上。调度器在匹配之前把名字复制成局部变量，正是为了
/// 让 `Captures` 借用那个局部量，而不是借用住在 `Ast` 里的名字——否则展开器同时
/// 要 `&mut Ast` 就借不出来了（`regex` 的 `Captures` 借用 haystack）。
#[derive(Debug)]
pub struct Matched<'h> {
    name: &'h str,
    captures: Option<Captures<'h>>,
}

impl<'h> Matched<'h> {
    /// 由调用名与可选的捕获组装。
    pub fn new(name: &'h str, captures: Option<Captures<'h>>) -> Self {
        Self { name, captures }
    }

    /// 没有名字可用时的命中信息（自然块就是这种情况）。
    pub const fn unnamed() -> Matched<'h> {
        Matched {
            name: "",
            captures: None,
        }
    }

    /// 完整调用名。自然块没有名字，此时为空串。
    pub const fn name(&self) -> &'h str {
        self.name
    }

    /// 正则**实际匹配到**的那一段；精确名命中时就是调用名。
    pub fn matched(&self) -> &'h str {
        match &self.captures {
            Some(captures) => captures.get(0).map_or(self.name, |m| m.as_str()),
            None => self.name,
        }
    }

    /// 第 `index` 个捕获组（`0` 是整段匹配）。
    ///
    /// 精确名命中时只有第 `0` 组存在（内容就是调用名）。
    pub fn capture(&self, index: usize) -> Option<&'h str> {
        match &self.captures {
            Some(captures) => captures.get(index).map(|m| m.as_str()),
            None if index == 0 => Some(self.name),
            None => None,
        }
    }

    /// 按名字取捕获组。
    pub fn capture_named(&self, name: &str) -> Option<&'h str> {
        self.captures.as_ref()?.name(name).map(|m| m.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn an_exact_hit_has_no_captures_but_still_reports_group_zero() {
        let matched = Matched::new("notice", None);

        assert_eq!(matched.name(), "notice");
        assert_eq!(matched.matched(), "notice");
        assert_eq!(matched.capture(0), Some("notice"));
        assert_eq!(matched.capture(1), None);
    }

    #[test]
    fn a_regex_hit_reports_what_actually_matched() {
        let name = "note-warning".to_string();
        let regex = Regex::new("^note-").unwrap();
        let captures = regex.captures(&name).unwrap();
        let matched = Matched::new(&name, Some(captures));

        // 整名与实际匹配到的片段不是一回事——这正是要区分的原因
        assert_eq!(matched.name(), "note-warning");
        assert_eq!(matched.matched(), "note-");
    }

    #[test]
    fn numbered_and_named_groups_are_both_reachable() {
        let name = "h3".to_string();
        let regex = Regex::new(r"^h(?P<level>[1-6])$").unwrap();
        let matched = Matched::new(&name, regex.captures(&name));

        assert_eq!(matched.matched(), "h3");
        assert_eq!(matched.capture(1), Some("3"));
        assert_eq!(matched.capture_named("level"), Some("3"));
        assert_eq!(matched.capture_named("nope"), None);
    }

    #[test]
    fn unnamed_is_what_natural_blocks_get() {
        let matched = Matched::unnamed();

        assert_eq!(matched.name(), "");
        assert_eq!(matched.matched(), "");
    }
}
