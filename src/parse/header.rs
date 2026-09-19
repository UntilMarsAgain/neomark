//! 调用块的头部识别。
//!
//! 一个调用行形如：
//!
//! ```text
//! ::name key=value flag :
//! ::name key=value: 首行内容
//! ```
//!
//! 规则：
//!
//! * 以 `::` 开头，`::` 之后到空白/`:`/`=` 为止是**调用名**；
//! * 其余以空白分隔的 token 是参数：`key=value` 为键值对，
//!   单独的 `flag` 视为 `flag=true`；
//! * **名字、键、值、链接目标四处都可以用双引号包裹**（引号内可以含空格与
//!   `:`），并支持 `\"` `\\` `\n` `\t` `\r` 转义。引号规则只有一处实现
//!   （[`unquote`]），调用头、行内调用的括号内部、链接目标共用它；
//! * **分隔冒号**把头部与首行内容分开：`::name k=v: 内容` 里的 `内容` 就是
//!   块体的第一行。分隔冒号定义为 `::` 之后第一个**不在引号内、且后面紧跟
//!   空白或行尾**的 `:`——所以 `url=http://x` 里的冒号不会被误认成分隔符，
//!   而 `::a url=http://x: 内容` 里 `x` 后面那个才是；
//! * 没有分隔冒号时整行都是头部（`::name k=v` 与旧的 `::name k=v:` 行为不变）；
//! * 解析永不失败：语义异常（空名字、引号不闭合、`=x` 之类）都按宽容策略处理。

use crate::ast::Params;

/// 头部识别结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallHeader {
    /// 调用名；`::` 之后没有名字时为空字符串。
    pub name: String,
    /// 参数表。
    pub params: Params,
    /// 分隔冒号之后的**首行内容**（已去掉前导空白）。
    ///
    /// 它是块体的第一行。没有分隔冒号、或者冒号后面是空的，就是 `None`。
    pub content: Option<String>,
}

/// 识别一行调用块头部。
///
/// 传入的应当是去掉行首缩进后的整行文本（`::` 开头）。为宽容起见，
/// 前导空白与缺失的 `::` 都会被容忍。
pub fn parse_call_header(line: &str) -> CallHeader {
    let (head, content) = match find_separator(line) {
        Some(separator) => (
            &line[..separator],
            Some(line[separator + 1..].trim_start().to_string()),
        ),
        None => (line, None),
    };

    let rest = head.trim_start();
    let rest = rest.strip_prefix("::").unwrap_or(rest);
    let rest = rest.trim();

    let mut scanner = Scanner::new(rest);
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

    CallHeader {
        name,
        params,
        content: content.filter(|text| !text.is_empty()),
    }
}

/// 找出**分隔冒号**的字节下标。
///
/// 它是 `::` 之后第一个不在引号内、且后面紧跟空白或行尾的 `:`。这条规则让
/// 值里的冒号安然无恙：`url=http://x` 的冒号后面是 `/`，所以不算分隔符。
fn find_separator(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let head_start = line.len() - trimmed.len();
    let scan_from = if trimmed.starts_with("::") {
        head_start + 2
    } else {
        head_start
    };

    let mut quoted = false;
    let mut escaped = false;

    for (offset, ch) in line[scan_from..].char_indices() {
        let at = scan_from + offset;

        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            ':' if !quoted => {
                let next = line[at + 1..].chars().next();
                if next.is_none_or(char::is_whitespace) {
                    return Some(at);
                }
            }
            _ => {}
        }
    }

    None
}

/// 把一个参数 token 转成键值对写进参数表。
fn push_token(params: &mut Params, token: &[char]) {
    match find_top_level_eq(token) {
        // `key=value`
        Some(index) if index > 0 => {
            let key = unquote(&token[..index]);
            let value = unquote(&token[index + 1..]);
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
/// [`unquote`] 的 `&str` 版本，给链接目标这类不是 token 的场景用。
pub(crate) fn unquote_str(text: &str) -> String {
    unquote(&text.chars().collect::<Vec<_>>())
}

/// 去掉包裹的双引号并处理 `\"` 转义；未闭合的引号按「到行尾为止」宽容处理。
///
/// **未被引号包裹时原样返回**，所以对任何 token 都能无脑调用。名字、键、值、
/// 以及链接目标全走这一个函数，引号规则因此只有一处。
pub(crate) fn unquote(token: &[char]) -> String {
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
    ///
    /// 名字**不支持**引号——名字是标识符，不是数据；只有键和值才需要引号。
    /// 读调用名：到空白、`:` 或 `=` 为止；也可以用双引号包裹。
    ///
    /// 引号内这些终止符都不算数、`\"` 按转义处理，所以 `::"a:b" k=v` 的名字是
    /// `a:b`。名字、键、值、链接目标**四处共用同一套引号规则**。
    fn read_name(&mut self) -> String {
        let mut raw = Vec::new();
        let mut in_quote = false;
        let mut escaped = false;

        while let Some(c) = self.peek() {
            if escaped {
                raw.push(c);
                self.bump();
                escaped = false;
                continue;
            }
            match c {
                '\\' if in_quote => {
                    raw.push(c);
                    self.bump();
                    escaped = true;
                }
                '"' => {
                    raw.push(c);
                    self.bump();
                    in_quote = !in_quote;
                }
                c if !in_quote && (c.is_whitespace() || c == ':' || c == '=') => break,
                _ => {
                    raw.push(c);
                    self.bump();
                }
            }
        }

        unquote(&raw)
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
    fn quoted_key_may_contain_spaces_and_colons() {
        let header = parse_call_header("::a \"my key\"=v");
        assert_eq!(header.params.get("my key"), Some("v"));

        // 键里的冒号不会被当成分隔冒号
        let header = parse_call_header("::a \"kind:type\"=v");
        assert_eq!(header.params.get("kind:type"), Some("v"));
    }

    #[test]
    fn key_and_value_may_both_be_quoted() {
        let header = parse_call_header("::a \"k 1\"=\"v 1\" \"k:2\"=\"v:2\"");
        assert_eq!(header.params, params(&[("k 1", "v 1"), ("k:2", "v:2")]));
        assert_eq!(header.content, None);
    }

    #[test]
    fn escapes_work_in_quoted_keys_too() {
        let header = parse_call_header("::a \"k\\\"1\"=v");
        assert_eq!(header.params.get("k\"1"), Some("v"));
    }

    #[test]
    fn a_quoted_key_still_needs_a_value_after_the_equals() {
        // `"k"=` 是「键是 k、值空」
        let header = parse_call_header("::a \"k\"=");
        assert_eq!(header.params.get("k"), Some(""));
    }

    #[test]
    fn names_may_be_quoted_too() {
        let header = parse_call_header("::\"my name\" k=v");
        assert_eq!(header.name, "my name");
        assert_eq!(header.params.get("k"), Some("v"));

        // 引号里的冒号不会被当成分隔冒号
        let header = parse_call_header("::\"a:b\": 内容");
        assert_eq!(header.name, "a:b");
        assert_eq!(header.content.as_deref(), Some("内容"));

        // 名字里的引号可以转义
        let header = parse_call_header("::\"a\\\"b\"");
        assert_eq!(header.name, "a\"b");
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
    fn a_colon_right_after_the_name_starts_the_first_line_instead_of_tolerating_a_param() {
        // 语义变化：以前 `::a: b=1` 会把 `b=1` 当参数（旧代码会剥掉尾部冒号），
        // 现在紧跟名字的分隔冒号一律开启首行内容——这才是它该有的意思。
        let header = parse_call_header("::a: b=1");
        assert_eq!(header.name, "a");
        assert!(header.params.is_empty());
        assert_eq!(header.content.as_deref(), Some("b=1"));
    }

    #[test]
    fn the_separator_colon_splits_off_the_first_line() {
        let header = parse_call_header("::a b=1: 首行");
        assert_eq!(header.name, "a");
        assert_eq!(header.params.get("b"), Some("1"));
        assert_eq!(header.content.as_deref(), Some("首行"));
    }

    #[test]
    fn a_colon_inside_a_value_is_not_the_separator() {
        // `http:` 的冒号后面是 `/`，所以不算分隔符
        let header = parse_call_header("::a url=http://x");
        assert_eq!(header.params.get("url"), Some("http://x"));
        assert_eq!(header.content, None);

        // `x` 后面那个才是
        let header = parse_call_header("::a url=http://x: 首行");
        assert_eq!(header.params.get("url"), Some("http://x"));
        assert_eq!(header.content.as_deref(), Some("首行"));
    }

    #[test]
    fn a_colon_inside_quotes_is_not_the_separator() {
        let header = parse_call_header("::a title=\"x: y\": 首行");
        assert_eq!(header.params.get("title"), Some("x: y"));
        assert_eq!(header.content.as_deref(), Some("首行"));
    }

    #[test]
    fn the_first_line_may_itself_contain_colons() {
        let header = parse_call_header("::a: 看 http://x 说完");
        assert_eq!(header.content.as_deref(), Some("看 http://x 说完"));
    }

    #[test]
    fn no_separator_or_an_empty_first_line_means_no_content() {
        assert_eq!(parse_call_header("::a b=1").content, None);
        // 旧的尾冒号写法仍然不产生内容
        assert_eq!(parse_call_header("::a b=1:").content, None);
        // 冒号后全是空白同样不算内容
        assert_eq!(parse_call_header("::a b=1:   ").content, None);
    }

    #[test]
    fn no_leading_whitespace_survives() {
        let header = parse_call_header("::a   b=1\t c=2");
        assert_eq!(header.params, params(&[("b", "1"), ("c", "2")]));
    }
}
