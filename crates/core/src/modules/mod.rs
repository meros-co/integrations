//! Native device modules, keyed by spec id.

use crate::module::{Module, OpenContext};

/// Build the module for a spec, or `None` if the core has no implementation
/// for it yet.
pub(crate) fn construct(spec: &str, context: OpenContext) -> Option<Box<dyn Module>> {
    let _ = context;
    match spec {
        _ => None,
    }
}
