//! t2123 (pipeline-prewarm; capabilities `plugins`, `generators`): every generator kind that draws on
//! the compute device names its shader (`Plugin::compute_shaders`); once the host has prepared what
//! the kinds name, the first frame of each kind creates no pipeline and dispatches none cold. A kind
//! running a shader it does not name turns this red. Its own test binary: the counters are
//! process-wide.

use kt_plugin_api::compute;

#[test]
fn t2123_a_prepared_generator_compiles_nothing_in_its_first_frame() {
    compute::install(kt_gpu::compute::open);
    let Some(host) = compute::host() else {
        eprintln!("skipped: no compute device");
        return;
    };
    let kinds: Vec<_> = generators_basic::plugins().into_iter().chain(generators_test::plugins()).collect();
    let mut named = 0;
    for p in &kinds {
        for s in p.compute_shaders() {
            host.prepare(&s).expect("prepared");
            named += 1;
        }
    }
    assert!(named >= 6, "the shader kinds name {named} shaders");
    for p in &kinds {
        let Some(g) = p.generator() else { continue };
        let m = p.manifest();
        let v: Vec<f64> = m.passport.iter().map(|s| s.default).collect();
        let (cold, made) = (kt_gpu::cold_draws(), kt_gpu::pipelines_in_frame());
        let _ = g.frame(&v, kt_time::Tick(0), 64, 36);
        assert_eq!(kt_gpu::pipelines_in_frame() - made, 0, "{}: its frame compiled a shader it does not name", m.id);
        assert_eq!(kt_gpu::cold_draws() - cold, 0, "{}: its frame dispatched a pipeline cold", m.id);
    }
}
