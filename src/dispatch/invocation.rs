//! 一次调用的完整视图：命中信息 + 参数。

use super::matched::Matched;
use crate::ast::Params;

/// 一次调用的**全部输入**：怎么匹配上的，以及作者写了哪些参数。
///
/// 展开器拿到的 [`Matched`] 只有「怎么匹配上的」（调用名、捕获组）；参数在节点里，
/// 得自己去 `ast.call(node).params` 取。想做「类名由某个参数算出来」这种通用包装时，
/// 两样都要，于是有了这个类型——它把两者合在一起，**借**着用，不复制。
///
/// ```
/// use neomark::dispatch::Invocation;
///
/// # fn demo(call: &Invocation<'_>) -> String {
/// // `::code language=rust` → class="language-rust"
/// format!("language-{}", call.param("language").unwrap_or("plaintext"))
/// # }
/// ```
///
/// 参数**保持字符串**：要不要解析某个值是展开器自己的事（见
/// [`Matched`](crate::dispatch::Matched) 的说明）。
#[derive(Debug)]
pub struct Invocation<'a> {
    matched: &'a Matched<'a>,
    params: &'a Params,
}

impl<'a> Invocation<'a> {
    /// 把命中信息与参数合起来。
    pub const fn new(matched: &'a Matched<'a>, params: &'a Params) -> Self {
        Self { matched, params }
    }

    /// 完整调用名。
    pub fn name(&self) -> &'a str {
        self.matched.name()
    }

    /// 正则**实际匹配到**的那一段；精确名命中时就是调用名。
    pub fn matched(&self) -> &'a str {
        self.matched.matched()
    }

    /// 第 `index` 个捕获组（`0` 是整段匹配）。
    pub fn capture(&self, index: usize) -> Option<&'a str> {
        self.matched.capture(index)
    }

    /// 按名字取捕获组。
    pub fn capture_named(&self, name: &str) -> Option<&'a str> {
        self.matched.capture_named(name)
    }

    /// 捕获组个数，含第 0 组。
    pub fn group_count(&self) -> usize {
        self.matched.group_count()
    }

    /// 按顺序遍历全部捕获组，含未参与匹配的 `None`。
    pub fn groups(&self) -> impl Iterator<Item = Option<&'a str>> + '_ {
        self.matched.groups()
    }

    /// 某个参数的值。
    pub fn param(&self, key: &str) -> Option<&'a str> {
        self.params.get(key)
    }

    /// 全部参数（顺序即书写顺序）。
    pub const fn params(&self) -> &'a Params {
        self.params
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn it_exposes_both_the_match_and_the_parameters() {
        let name = "h3".to_string();
        let captures = Regex::new(r"^h(?P<level>[1-6])$").unwrap().captures(&name);
        let matched = Matched::new(&name, captures);

        let mut params = Params::new();
        params.push("language", "rust");

        let call = Invocation::new(&matched, &params);

        assert_eq!(call.name(), "h3");
        assert_eq!(call.matched(), "h3");
        assert_eq!(call.capture(1), Some("3"));
        assert_eq!(call.capture_named("level"), Some("3"));
        assert_eq!(call.group_count(), 2);
        assert_eq!(call.param("language"), Some("rust"));
        assert_eq!(call.param("nope"), None);
        assert_eq!(call.params().len(), 1);
    }

    #[test]
    fn an_exact_hit_still_works() {
        let name = "quote".to_string();
        let matched = Matched::new(&name, None);
        let params = Params::new();

        let call = Invocation::new(&matched, &params);

        assert_eq!(call.matched(), "quote");
        assert_eq!(call.capture(1), None);
        assert_eq!(call.param("origin"), None);
    }
}
