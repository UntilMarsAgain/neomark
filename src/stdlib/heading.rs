//! 标题：`::h1` ~ `::h6`。

use crate::dispatch::Registry;
use crate::handlers::{NaturalExpander, Wrap};

/// 标题模式：`h` 加一个 1~6 的数字。
///
/// 一条模式覆盖六级标题，级别由正则咬住，所以 `ha`、`h7` 根本不会命中这里，
/// 而是落到兜底展开器上报错。
pub const PATTERN: &str = "^h[1-6]$";

/// 注册标题展开器。
///
/// 标题**没有专门的展开器**——它就是个[包装器](Wrap)：
///
/// * 标签名取正则实际匹配到的那一段（`h3`）；
/// * 类名是同一个片断加 `nm-` 前缀（`nm-h3`）；
/// * 正文按**行内**解析，因为 `<h3>` 里不能有 `<p>`。
pub fn register(registry: &mut Registry, natural: NaturalExpander) {
    registry.register(
        regex::Regex::new(PATTERN).expect("内置标题模式在测试里被验证过"),
        Wrap::tag_from_match()
            .class_prefix("nm-")
            .class_from_match()
            .inline(natural),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pattern_covers_exactly_h1_to_h6() {
        let regex = regex::Regex::new(PATTERN).unwrap();

        for name in ["h1", "h2", "h3", "h4", "h5", "h6"] {
            assert!(regex.is_match(name), "{name}");
        }
        for name in ["h0", "h7", "ha", "h", "h1x", "xh1"] {
            assert!(!regex.is_match(name), "{name}");
        }
    }
}
