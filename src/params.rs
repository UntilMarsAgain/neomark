//! 调用块的参数表。
//!
//! 参数键与值**一律视为字符串**，当前不做类型区分。
//! 无值参数（例如 `lazy`）在解析阶段即被规范化为值 `"true"`，
//! 也就是说 `lazy` 与 `lazy=true` 等价。

use std::ops::Deref;

/// 调用块的参数表：按出现顺序保存的字符串键值对。
///
/// 重复键会全部保留；[`Params::get`] 返回**第一个**匹配项。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Params {
    entries: Vec<(String, String)>,
}

impl Params {
    /// 空参数表。
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// 追加一个键值对。
    pub fn push(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.push((key.into(), value.into()));
    }

    /// 取第一个名为 `key` 的参数值。
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// 是否存在名为 `key` 的参数。
    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    /// 参数个数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否没有任何参数。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 以切片形式访问全部参数。
    pub fn entries(&self) -> &[(String, String)] {
        &self.entries
    }

    /// 按出现顺序遍历参数。
    pub fn iter(&self) -> std::slice::Iter<'_, (String, String)> {
        self.entries.iter()
    }
}

impl Deref for Params {
    type Target = [(String, String)];

    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl FromIterator<(String, String)> for Params {
    fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
        Self {
            entries: iter.into_iter().collect(),
        }
    }
}

impl<'a> IntoIterator for &'a Params {
    type Item = &'a (String, String);
    type IntoIter = std::slice::Iter<'a, (String, String)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}
