//! HTML 实体引用。
//!
//! 支持全部**数字实体**（`&#35;` / `&#x22;`），以及一份**常用命名实体**表。
//! 之所以是「常用」而不是 CommonMark 要求的那两千多个：完整表有 2200+ 条，
//! 塞进源码会让仓库臃肿几十 KB。未收录的名字会**原样保留**（这也正是
//! CommonMark 对非法实体的行为），将来要补全只需替换本文件。

/// 解析 `&` 开头的一段。
///
/// 成功时返回（替换文本, 吃掉的字符数），字符数**包含**开头的 `&` 与结尾的 `;`。
pub(crate) fn parse(chars: &[char]) -> Option<(String, usize)> {
    if chars.first() != Some(&'&') {
        return None;
    }

    let semicolon = chars.iter().position(|&c| c == ';')?;
    // 实体名不可能太长，挡掉病态输入。
    if semicolon > 32 {
        return None;
    }

    let name: String = chars[1..semicolon].iter().collect();
    let consumed = semicolon + 1;

    if let Some(digits) = name.strip_prefix('#') {
        let code = if let Some(hex) = digits.strip_prefix(['x', 'X']) {
            u32::from_str_radix(hex, 16).ok()?
        } else {
            digits.parse::<u32>().ok()?
        };
        // CommonMark：非法码位与 U+0000 都换成替换字符。
        let ch = match code {
            0 => '\u{fffd}',
            _ => char::from_u32(code).unwrap_or('\u{fffd}'),
        };
        return Some((ch.to_string(), consumed));
    }

    lookup(&name).map(|text| (text.to_string(), consumed))
}

/// 常用命名实体表。
fn lookup(name: &str) -> Option<&'static str> {
    Some(match name {
        // 基础与拉丁补充
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{a0}",
        "iexcl" => "¡",
        "cent" => "¢",
        "pound" => "£",
        "curren" => "¤",
        "yen" => "¥",
        "brvbar" => "¦",
        "sect" => "§",
        "uml" => "¨",
        "copy" => "©",
        "ordf" => "ª",
        "laquo" => "«",
        "not" => "¬",
        "shy" => "\u{ad}",
        "reg" => "®",
        "macr" => "¯",
        "deg" => "°",
        "plusmn" => "±",
        "sup2" => "²",
        "sup3" => "³",
        "acute" => "´",
        "micro" => "µ",
        "para" => "¶",
        "middot" => "·",
        "cedil" => "¸",
        "sup1" => "¹",
        "ordm" => "º",
        "raquo" => "»",
        "frac14" => "¼",
        "frac12" => "½",
        "frac34" => "¾",
        "iquest" => "¿",
        "times" => "×",
        "divide" => "÷",
        "fnof" => "ƒ",
        "OElig" => "Œ",
        "oelig" => "œ",
        "Scaron" => "Š",
        "scaron" => "š",
        "Yuml" => "Ÿ",
        // 标点与排版
        "circ" => "ˆ",
        "tilde" => "˜",
        "ensp" => "\u{2002}",
        "emsp" => "\u{2003}",
        "thinsp" => "\u{2009}",
        "zwnj" => "\u{200c}",
        "zwj" => "\u{200d}",
        "lrm" => "\u{200e}",
        "rlm" => "\u{200f}",
        "ndash" => "–",
        "mdash" => "—",
        "lsquo" => "‘",
        "rsquo" => "’",
        "sbquo" => "‚",
        "ldquo" => "“",
        "rdquo" => "”",
        "bdquo" => "„",
        "dagger" => "†",
        "Dagger" => "‡",
        "bull" => "•",
        "hellip" => "…",
        "permil" => "‰",
        "prime" => "′",
        "Prime" => "″",
        "lsaquo" => "‹",
        "rsaquo" => "›",
        "oline" => "‾",
        "frasl" => "⁄",
        "euro" => "€",
        "trade" => "™",
        // 希腊字母
        "Alpha" => "Α",
        "Beta" => "Β",
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Epsilon" => "Ε",
        "Zeta" => "Ζ",
        "Eta" => "Η",
        "Theta" => "Θ",
        "Iota" => "Ι",
        "Kappa" => "Κ",
        "Lambda" => "Λ",
        "Mu" => "Μ",
        "Nu" => "Ν",
        "Xi" => "Ξ",
        "Omicron" => "Ο",
        "Pi" => "Π",
        "Rho" => "Ρ",
        "Sigma" => "Σ",
        "Tau" => "Τ",
        "Upsilon" => "Υ",
        "Phi" => "Φ",
        "Chi" => "Χ",
        "Psi" => "Ψ",
        "Omega" => "Ω",
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" => "ε",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" => "θ",
        "iota" => "ι",
        "kappa" => "κ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "omicron" => "ο",
        "pi" => "π",
        "rho" => "ρ",
        "sigmaf" => "ς",
        "sigma" => "σ",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" => "φ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        "thetasym" => "ϑ",
        "upsih" => "ϒ",
        "piv" => "ϖ",
        // 数学与箭头
        "alefsym" => "ℵ",
        "weierp" => "℘",
        "image" => "ℑ",
        "real" => "ℜ",
        "larr" => "←",
        "uarr" => "↑",
        "rarr" => "→",
        "darr" => "↓",
        "harr" => "↔",
        "crarr" => "↵",
        "lArr" => "⇐",
        "uArr" => "⇑",
        "rArr" => "⇒",
        "dArr" => "⇓",
        "hArr" => "⇔",
        "forall" => "∀",
        "part" => "∂",
        "exist" => "∃",
        "empty" => "∅",
        "nabla" => "∇",
        "isin" => "∈",
        "notin" => "∉",
        "ni" => "∋",
        "prod" => "∏",
        "sum" => "∑",
        "minus" => "−",
        "lowast" => "∗",
        "radic" => "√",
        "prop" => "∝",
        "infin" => "∞",
        "ang" => "∠",
        "and" => "∧",
        "or" => "∨",
        "cap" => "∩",
        "cup" => "∪",
        "int" => "∫",
        "there4" => "∴",
        "sim" => "∼",
        "cong" => "≅",
        "asymp" => "≈",
        "ne" => "≠",
        "equiv" => "≡",
        "le" => "≤",
        "ge" => "≥",
        "sub" => "⊂",
        "sup" => "⊃",
        "nsub" => "⊄",
        "sube" => "⊆",
        "supe" => "⊇",
        "oplus" => "⊕",
        "otimes" => "⊗",
        "perp" => "⊥",
        "sdot" => "⋅",
        "lceil" => "⌈",
        "rceil" => "⌉",
        "lfloor" => "⌊",
        "rfloor" => "⌋",
        "lang" => "⟨",
        "rang" => "⟩",
        "loz" => "◊",
        "spades" => "♠",
        "clubs" => "♣",
        "hearts" => "♥",
        "diams" => "♦",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(text: &str) -> Option<(String, usize)> {
        parse(&text.chars().collect::<Vec<_>>())
    }

    #[test]
    fn numeric_entities() {
        assert_eq!(parse_str("&#35;").unwrap().0, "#");
        assert_eq!(parse_str("&#x22;").unwrap().0, "\"");
        assert_eq!(parse_str("&#x1F600;").unwrap().0, "😀");
        assert_eq!(parse_str("&#35;").unwrap().1, 5);
    }

    #[test]
    fn named_entities() {
        assert_eq!(parse_str("&amp;").unwrap().0, "&");
        assert_eq!(parse_str("&hellip;").unwrap().0, "…");
        assert_eq!(parse_str("&alpha;").unwrap().0, "α");
    }

    #[test]
    fn unknown_and_malformed_entities_are_left_alone() {
        assert!(parse_str("&nope;").is_none());
        assert!(parse_str("&amp").is_none()); // 缺分号
        assert!(parse_str("& #35;").is_none());
        assert!(parse_str("&#xZZ;").is_none());
    }

    #[test]
    fn invalid_code_points_become_the_replacement_character() {
        assert_eq!(parse_str("&#0;").unwrap().0, "\u{fffd}");
        assert_eq!(parse_str("&#xD800;").unwrap().0, "\u{fffd}"); // 代理对
        assert_eq!(parse_str("&#x110000;").unwrap().0, "\u{fffd}"); // 超出范围
    }
}
