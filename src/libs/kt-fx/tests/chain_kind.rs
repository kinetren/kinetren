//! t1057 (plugin-api-barriers, found in stage 3; capability `audio-render`, invariant 3): the
//! fingerprint of an audio chain's STRUCTURE tells plugin kinds apart.
//!
//! `chain_fingerprint` decides whether `FxStateStore` keeps a clip's processor (the same structure
//! → the same state objects, values through `apply_params`) or rebuilds it. It hashed the enum
//! variant only, and every plugin slot is the one `Plugin` variant: replacing an EQ with a filter in
//! the same slot kept the EQ's node, which then received the filter's values.

use kt_model::FxInstance;

#[test]
fn t1057_a_chain_fingerprint_tells_plugin_kinds_apart() {
    kt_plugins_builtin::install();
    let fp = kt_fx::chain_fingerprint;
    let (eq, filter, delay) = (FxInstance::builtin("eq"), FxInstance::builtin("filter"), FxInstance::builtin("delay"));
    assert_ne!(fp(std::slice::from_ref(&eq)), fp(std::slice::from_ref(&filter)), "an EQ and a filter in one slot share a structure");
    assert_ne!(fp(&[eq.clone(), delay.clone()]), fp(&[delay.clone(), eq.clone()]), "the order of kinds is the structure");
    // the VALUES are not the structure: they arrive through apply_params, the state survives
    let mut louder = filter.clone();
    assert!(louder.set_param("cutoff_hz", 2400.0), "the filter has no cutoff row");
    assert_eq!(fp(std::slice::from_ref(&filter)), fp(std::slice::from_ref(&louder)), "a value edit rebuilt the structure");
}
