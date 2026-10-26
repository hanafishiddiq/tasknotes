//! Bulk changes: preview first, then confirm with the preview's token.

use crate::tools::ToolDef;

#[derive(Default)]
pub struct Previews {}

pub fn tools() -> Vec<ToolDef> {
    Vec::new()
}
