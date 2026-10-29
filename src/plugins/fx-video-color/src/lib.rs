//! Colour video effects as plugins (plugin-api-stage1): color_grade, duotone, posterize, vignette.
//! Each moves here from kt-model/kt-fx in its stage; the crate is registered before its first
//! plugin so the graph rule (t975) holds from the start.

use kt_plugin_api::Plugin;

/// The plugins of this family, in the assembler's order.
pub fn plugins() -> Vec<std::sync::Arc<dyn Plugin>> {
    Vec::new()
}
