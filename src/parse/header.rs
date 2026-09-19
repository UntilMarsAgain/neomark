//! 调用块的头部识别。
//!
//! 一个调用行形如：
//!
//! ```text
//! ::name key=value flag "带空格的值"=... :
//! ```
//!
//! 规则：
//!
//! * 以 `::` 开头，`::` 之后到空白/`:`/`=` 为止是**调用名**；
//! * 其余以空白分隔的 token 是参数：`key=value` 为键值对，
//!   单独的 `flag` 视为 `flag=true`；
//! * 值可以用双引号包裹（可以含空格），支持 `\"` `\\` `\n` `\t` `\r` 转义；
//! * 行尾的 `:` 是可选终止符，会被剥离；
//! * 解析永不失败：语义异常（空名字、引号不闭合、`=x` 之类）都按宽容策略处理。

use crate::ast::Params;

/// 头部识别结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallHeader {
    /// 调用名；`::` 之后没有名字时为空字符串。
    pub name: String,
    /// 参数表。
    pub params: Params,
}

/// 识别一行调用块头部。
///
/// 传入的应当是去掉行首缩进后的整行文本（`::` 开头）。为宽容起见，
/// 前导空白与缺失的 `::` 都会被容忍。
pub fn parse_call_header(line: &str) -> CallHeader {
    let rest = line.trim_start();
    let rest = rest.strip_prefix("::").unwrap_or(rest);

    // 末尾的可选终止符 `:` 先剥离，再做 token 切分。
    let rest = rest.trim();
    let body = rest.strip_suffix(':').unwrap_or(rest);

    let mut scanner = Scanner::new(body);
    let name = scanner.read_name();

    // 容忍 `::name: param` 这种名字后面多出来的冒号。
    while scanner.peek() == Some(':') {
        scanner.bump();
    }

    let mut params = Params::new();
    loop {
        scanner.skip_whitespace();
        match scanner.peek() {
            None => break,
            Some(':') => {
                scanner.bump();
            }
            Some(_) => {
                let token = scanner.read_token();
                if token.is_empty() {
                    scanner.bump();
                    continue;
                }
                push_token(&mut params, &token);
            }
        }
    }

    CallHeader { name, params }
}

/// 把一个参数 token 转成键值对写进参数表。
fn push_token(params: &mut Params, token: &[char]) {
    match find_top_level_eq(token) {
        // `key=value`
        Some(index) if index > 0 => {
            let key: String = token[..index].iter().collect();
            let value = parse_value(&token[index + 1..]);
            params.push(key, value);
        }
        // 无值参数，等价于 `flag=true`；`=x` 这类异常 token 也走这里（整个 token 做键）。
        _ => {
            let key = unquote(token);
            params.push(key, "true");
        }
    }
}

/// 找到不在引号内的第一个 `=`。
fn find_top_level_eq(token: &[char]) -> Option<usize> {
    let mut in_quote = false;
    let mut escaped = false;
    for (index, &c) in token.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if in_quote => escaped = true,
            '"' => in_quote = !in_quote,
            '=' if !in_quote => return Some(index),
            _ => {}
        }
    }
    None
}

/// 解析参数值：带引号则去引号并处理转义，否则按字面取用。
fn parse_value(value: &[char]) -> String {
    if value.first() == Some(&'"') {
        unquote(value)
    } else {
        value.iter().collect()
    }
}

/// 去掉包裹的双引号并处理转义；未闭合的引号按“到行尾为止”宽容处理。
fn unquote(token: &[char]) -> String {
    let quoted = token.first() == Some(&'"');
    let mut out = String::new();
    let mut iter = token.iter().copied().peekable();

    if quoted {
        iter.next();
    }
    let mut in_quote = quoted;
    while let Some(c) = iter.next() {
        if !in_quote {
            // 引号已闭合，后面的残余字符按字面保留。
            out.push(c);
            continue;
        }
        match c {
            '"' => in_quote = false,
            '\\' => match iter.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            },
            _ => out.push(c),
        }
    }
    out
}

/// 头部行上的字符扫描器。
struct Scanner {
    chars: Vec<char>,
    pos: usize,
}

impl Scanner {
    fn new(text: &str) -> Self {
        Self {
            chars: text.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }

    /// 读调用名：到空白、`:` 或 `=` 为止。
    fn read_name(&mut self) -> String {
        let mut name = String::new();
        while let Some(c) = self.peek() {
            if c.is_whitespace() || c == ':' || c == '=' {
                break;
            }
            name.push(c);
            self.bump();
        }
        name
    }

    /// 读一个参数 token：到引号外的空白为止，引号内的空白保留。
    fn read_token(&mut self) -> Vec<char> {
        let mut out = Vec::new();
        let mut in_quote = false;
        let mut escaped = false;
        while let Some(c) = self.peek() {
            if escaped {
                out.push(c);
                self.bump();
                escaped = false;
                continue;
            }
            match c {
                '\\' if in_quote => {
                    out.push(c);
                    self.bump();
                    escaped = true;
                }
                '"' => {
                    out.push(c);
                    self.bump();
                    in_quote = !in_quote;
                }
                c if c.is_whitespace() && !in_quote => break,
                _ => {
                    out.push(c);
                    self.bump();
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 把 `(键, 值)` 数组变成参数表，方便断言。
    fn params(pairs: &[(&str, &str)]) -> Params {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn name_and_key_value_params() {
        let header = parse_call_header("::code lang=neomark:");
        assert_eq!(header.name, "code");
        assert_eq!(header.params, params(&[("lang", "neomark")]));
    }

    #[test]
    fn trailing_colon_is_optional() {
        assert_eq!(
            parse_call_header("::code lang=neomark"),
            parse_call_header("::code lang=neomark:")
        );
    }

    #[test]
    fn bare_flag_becomes_true() {
        let header = parse_call_header("::image width=300 height=200 lazy:");
        assert_eq!(
            header.params,
            params(&[("width", "300"), ("height", "200"), ("lazy", "true")])
        );
    }

    #[test]
    fn quoted_value_may_contain_spaces() {
        let header = parse_call_header("::notice title=\"Be Careful!\":");
        assert_eq!(header.name, "notice");
        assert_eq!(header.params.get("title"), Some("Be Careful!"));
    }

    #[test]
    fn quoted_value_may_contain_unicode_and_punctuation() {
        let header = parse_call_header("::quote author=\"张三\" source=\"《文章标题》\":");
        assert_eq!(
            header.params,
            params(&[("author", "张三"), ("source", "《文章标题》")])
        );
    }

    #[test]
    fn escapes_inside_quotes() {
        let header = parse_call_header("::a t=\"a\\\"b\\\\c\"");
        assert_eq!(header.params.get("t"), Some("a\"b\\c"));
    }

    #[test]
    fn equals_sign_inside_quotes_is_part_of_the_value() {
        let header = parse_call_header("::a t=\"x=y\"");
        assert_eq!(header.params.get("t"), Some("x=y"));
    }

    #[test]
    fn extra_equals_signs_stay_in_the_value() {
        let header = parse_call_header("::a b=c=d");
        assert_eq!(header.params.get("b"), Some("c=d"));
    }

    #[test]
    fn a_lone_double_colon_has_an_empty_name() {
        let header = parse_call_header("::");
        assert_eq!(header.name, "");
        assert!(header.params.is_empty());
    }

    #[test]
    fn malformed_input_is_parsed_leniently() {
        // 引号不闭合：按“到行尾为止”取值。
        let header = parse_call_header("::a t=\"未闭合");
        assert_eq!(header.params.get("t"), Some("未闭合"));

        // `=x` 这种异常 token：整个 token 当键，值为 true。
        let header = parse_call_header("::a =x");
        assert_eq!(header.params.get("=x"), Some("true"));
    }

    #[test]
    fn repeated_keys_are_all_kept_and_get_returns_the_first() {
        let header = parse_call_header("::a x=1 x=2");
        assert_eq!(header.params.len(), 2);
        assert_eq!(header.params.get("x"), Some("1"));
    }

    #[test]
    fn colon_right_after_the_name_is_tolerated() {
        let header = parse_call_header("::a: b=1");
        assert_eq!(header.name, "a");
        assert_eq!(header.params.get("b"), Some("1"));
    }

    #[test]
    fn no_leading_whitespace_survives() {
        let header = parse_call_header("::a   b=1\t c=2");
        assert_eq!(header.params, params(&[("b", "1"), ("c", "2")]));
    }
}
