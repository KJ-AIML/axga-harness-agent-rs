//! Tool registry — a name-indexed collection of tools.

use super::Tool;
use std::collections::HashMap;
use std::sync::Arc;

/// Registry mapping tool names to tool implementations.
#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool. Returns `Err` if a tool with the same name already exists.
    pub fn register(&mut self, tool: impl Tool + 'static) -> Result<(), axga_shared::error::AxgaError> {
        let name = tool.name().to_string();
        if self.tools.contains_key(&name) {
            return Err(axga_shared::error::AxgaError::ToolError {
                tool: name,
                message: "tool already registered".into(),
            });
        }
        self.tools.insert(name, Arc::new(tool));
        Ok(())
    }

    /// Look up a tool by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// List all registered tool names.
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.tools.keys()
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}
