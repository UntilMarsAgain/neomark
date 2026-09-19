//! Emoji 短码 `:name:` 的**语法**识别。
//!
//! 这里只回答两件事：形状像不像短码、名字是什么。**不查表、不决定长什么样。**
//! `:rocket:` 输出 `🚀` 还是手搓的 SVG，是渲染器的事（见 `crate::html::emoji`）。
//! 这样将来换图标、加属性、加 title，解析层一行都不用动。
//!
//! 只认「小写字母 / 数字 / `_` `+` `-`」组成、两侧都有冒号、且名字至少两个
//! 字符的短码，所以正文里的 `12:30:45` 不会被误判。

/// 认出一个短码，返回（名字, 吃掉的字符数），字符数包含两侧的冒号。
pub(crate) fn parse(chars: &[char]) -> Option<(String, usize)> {
    if chars.first() != Some(&':') {
        return None;
    }

    // 从开头的冒号之后找收尾的冒号；短码名不可能太长。
    let end = 1 + chars[1..].iter().take(48).position(|&c| c == ':')?;
    if end < 3 {
        // 名字至少两个字符，顺带挡掉 `::`。
        return None;
    }

    let name: String = chars[1..end].iter().collect();
    if !name
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'+' | b'-'))
    {
        return None;
    }

    Some((name, end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(text: &str) -> Option<(String, usize)> {
        parse(&text.chars().collect::<Vec<_>>())
    }

    #[test]
    fn recognises_the_syntax_and_reports_the_name() {
        assert_eq!(parse_str(":smile:").unwrap(), ("smile".to_string(), 7));
        assert_eq!(parse_str(":+1:").unwrap(), ("+1".to_string(), 4));
        assert_eq!(parse_str(":-1:").unwrap(), ("-1".to_string(), 4));
        assert_eq!(parse_str(":x_y:").unwrap(), ("x_y".to_string(), 5));
    }

    #[test]
    fn unknown_names_are_still_syntax_this_layer_does_not_look_them_up() {
        // `:nope:` 形状合法，名字照收——认不认识是渲染器的事。
        assert_eq!(parse_str(":nope:").unwrap(), ("nope".to_string(), 6));
        assert_eq!(parse_str(":30:").unwrap(), ("30".to_string(), 4));
    }

    #[test]
    fn malformed_shapes_are_rejected() {
        assert!(parse_str(":Smile:").is_none()); // 大写
        assert!(parse_str(":smile").is_none()); // 缺右冒号
        assert!(parse_str(":").is_none());
        assert!(parse_str(":a:").is_none()); // 名字太短
        assert!(parse_str("::x::").is_none()); // `::` 是块标记
        assert!(parse_str(":中文:").is_none());
    }
}
