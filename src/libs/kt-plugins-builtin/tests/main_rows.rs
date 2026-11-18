//! t2998 (fx-graph D4a; capability `plugins`): every built-in plugin has a valid main set — at most
//! three rows, each a number or a choice of its passport — and both halves of every simulated device
//! name theirs; a paired video kind without one is refused at registration.

use kt_plugin_api::{Manifest, ParamType, Plugin, Registry, main_rows};
use std::sync::Arc;

#[test]
fn t2998_every_built_in_plugin_has_a_valid_main_set() {
    let reg = kt_plugins_builtin::install();
    for p in reg.plugins() {
        let m = p.manifest();
        let rows = main_rows(m);
        assert!(rows.len() <= kt_plugin_api::MAX_MAIN_ROWS, "{}: {} main rows", m.id, rows.len());
        for r in &rows {
            let spec = m.passport.iter().find(|s| s.name == *r).unwrap_or_else(|| panic!("{}: no row {r}", m.id));
            assert!(matches!(spec.ty, ParamType::Num | ParamType::Choice { .. }), "{}: {r} is not a slider or a choice", m.id);
        }
        if reg.counterpart(&m.id).is_some() {
            assert!(!m.ui.main.is_empty(), "{}: a half of a simulated device names no main rows", m.id);
        }
    }
    // a paired video kind without its main rows is refused before any frame
    struct NoMain(Arc<dyn Plugin>, Manifest);
    impl Plugin for NoMain {
        fn manifest(&self) -> &Manifest {
            &self.1
        }
        fn barrier(&self) -> Option<&dyn kt_plugin_api::barrier::BarrierKernel> {
            self.0.barrier()
        }
    }
    let tape = reg.get(kt_plugin_api::Family::Fx, "vhs_tape").expect("vhs_tape").clone();
    let mut m = tape.manifest().clone();
    m.id = "test_tape".into();
    m.ui.main = &[];
    let r = Registry::new().register(Arc::new(NoMain(tape.clone(), m.clone())));
    assert!(r.is_err_and(|e| e.reason.contains("main rows")), "a device without main rows was registered");
    // and a main row the passport lacks
    m.ui.main = &["no_such_row"];
    let r = Registry::new().register(Arc::new(NoMain(tape, m)));
    assert!(r.is_err_and(|e| e.reason.contains("lacks")), "a main row outside the passport was registered");
}
