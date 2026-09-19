//! 展开器的运行期注册表。

use std::collections::HashMap;

use super::handler::{Fallback, Handler};

/// 展开器注册表。
///
/// * 调用块按**调用名**查找；
/// * 自然块查找专门的**自然块展开器**；
/// * 两者都没命中时，用**兜底展开器**，它默认把块的原文内容放进报错节点。
///
/// 同名重复注册会覆盖旧的展开器。
pub struct Registry {
    handlers: HashMap<String, Box<dyn Handler>>,
    natural: Option<Box<dyn Handler>>,
    fallback: Box<dyn Handler>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            handlers: HashMap::new(),
            natural: None,
            fallback: Box::new(Fallback),
        }
    }
}

impl Registry {
    /// 空注册表；兜底展开器为 [`Fallback`]。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册（或覆盖）一个**调用名**的展开器。
    pub fn register(
        &mut self,
        name: impl Into<String>,
        handler: impl Handler + 'static,
    ) -> &mut Self {
        self.handlers.insert(name.into(), Box::new(handler));
        self
    }

    /// 注册**自然块**展开器。
    pub fn register_natural(&mut self, handler: impl Handler + 'static) -> &mut Self {
        self.natural = Some(Box::new(handler));
        self
    }

    /// 替换**兜底**展开器。
    pub fn set_fallback(&mut self, handler: impl Handler + 'static) -> &mut Self {
        self.fallback = Box::new(handler);
        self
    }

    /// 按调用名查找展开器。
    pub fn get(&self, name: &str) -> Option<&dyn Handler> {
        self.handlers.get(name).map(|handler| handler.as_ref())
    }

    /// 自然块展开器；没注册过时为 `None`。
    pub fn natural(&self) -> Option<&dyn Handler> {
        self.natural.as_deref()
    }

    /// 兜底展开器。
    pub fn fallback(&self) -> &dyn Handler {
        self.fallback.as_ref()
    }

    /// 是否注册过这个调用名。
    pub fn contains(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    /// 已注册的调用名个数（不含自然块与兜底）。
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// 是否没有任何按名注册的展开器。
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    /// 已注册的调用名，顺序不定。
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.handlers.keys().map(String::as_str)
    }
}
