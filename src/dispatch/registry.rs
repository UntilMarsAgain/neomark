//! 调用名到展开器的运行期注册表。

use std::collections::HashMap;

use super::handler::Handler;

/// 调用名 → 展开器的运行期注册表。
///
/// 同名重复注册会覆盖旧的展开器。
#[derive(Default)]
pub struct Registry {
    handlers: HashMap<String, Box<dyn Handler>>,
}

impl Registry {
    /// 空注册表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册（或覆盖）一个名字。
    pub fn register(
        &mut self,
        name: impl Into<String>,
        handler: impl Handler + 'static,
    ) -> &mut Self {
        self.handlers.insert(name.into(), Box::new(handler));
        self
    }

    /// 查找展开器。
    pub fn get(&self, name: &str) -> Option<&dyn Handler> {
        self.handlers.get(name).map(|handler| handler.as_ref())
    }

    /// 是否注册过这个名字。
    pub fn contains(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    /// 已注册的展开器个数。
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// 是否没有任何展开器。
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    /// 已注册的名字，顺序不定。
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.handlers.keys().map(String::as_str)
    }
}
