//! 注册时用的**查找键**。

use regex::Regex;

/// 怎么把一个调用名认领给展开器。
///
/// 注册表只留**一个**注册入口 [`register`](crate::dispatch::Registry::register)，
/// 键写成什么就是什么匹配：
///
/// ```
/// use neomark::{Registry, handlers, regex::Regex};
///
/// let mut registry = Registry::new();
///
/// // 精确名：`&str` / `String` 直接就能当键
/// registry.register("notice", handlers::Wrap::tag("div").class("nm-notice"));
///
/// // 正则模式：把 `Regex` 当键
/// registry.register(
///     Regex::new("^h[1-6]$").unwrap(),
///     handlers::Wrap::tag_from_match().class_from_match(),
/// );
/// ```
///
/// # 为什么是个类型
///
/// 以后要加新的认领方式（优先级、谓词、行内专用命名空间……）时，**加一个枚举
/// 分支即可，不必再加一个 `register_xxx` 方法**。展开器会拿到
/// [`Matched`](crate::dispatch::Matched)，所以新键形式也能把自己的命中信息带过去。
///
/// 枚举标了 `#[non_exhaustive]`，所以外部代码匹配它时**必须**写 `_` 兜底——
/// 这样将来加分支不会破坏别人的编译。
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Key {
    /// 精确调用名。查表命中，优先级最高。
    Name(String),
    /// 正则模式。整体由模式自己决定锚不锚，见
    /// [`Registry`](crate::dispatch::Registry#正则模式) 的说明。
    Pattern(Regex),
}

impl Key {
    /// 用字符串模式构造；模式非法时返回错误。
    ///
    /// 直接 `Regex::new(..).unwrap()` 当键也行，这个只是省一次 `unwrap`。
    pub fn pattern(pattern: &str) -> Result<Self, regex::Error> {
        Regex::new(pattern).map(Key::Pattern)
    }

    /// 这个键的原文：精确名就是名字，正则就是模式串。
    ///
    /// 也用于「同一个键重复注册就覆盖」的判定。
    pub fn as_str(&self) -> &str {
        match self {
            Key::Name(name) => name,
            Key::Pattern(regex) => regex.as_str(),
        }
    }

    /// 是否是精确名。
    pub const fn is_name(&self) -> bool {
        matches!(self, Key::Name(_))
    }
}

impl From<&str> for Key {
    fn from(name: &str) -> Self {
        Key::Name(name.to_string())
    }
}

impl From<String> for Key {
    fn from(name: String) -> Self {
        Key::Name(name)
    }
}

impl From<&String> for Key {
    fn from(name: &String) -> Self {
        Key::Name(name.clone())
    }
}

impl From<Regex> for Key {
    fn from(regex: Regex) -> Self {
        Key::Pattern(regex)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_become_exact_names() {
        assert_eq!(Key::from("notice").as_str(), "notice");
        assert_eq!(Key::from(String::from("notice")).as_str(), "notice");
        assert_eq!(Key::from(&String::from("notice")).as_str(), "notice");
        assert!(Key::from("notice").is_name());
    }

    #[test]
    fn regexes_become_patterns() {
        let key = Key::from(Regex::new("^h[1-6]$").unwrap());

        assert_eq!(key.as_str(), "^h[1-6]$");
        assert!(!key.is_name());
    }

    #[test]
    fn the_string_pattern_constructor_reports_bad_patterns() {
        assert!(Key::pattern("h[1-6]").is_ok());
        assert!(Key::pattern("h[").is_err());
    }
}
