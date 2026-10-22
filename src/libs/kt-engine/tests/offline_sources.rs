//! Test 422 (R.49 B11): the new wording of invariant 5 — a guard of the phrasing.

/// Invariant 5 was rewritten: “all clients go through the MediaWorker queue” was
/// untrue — there are four of its own short-lived sources in the tree. The test guards
/// the TEXT of the rule and its conditions: if the wording is reverted to the old one, it
/// turns red rather than staying silent.
///
/// Kin to “catalog keys nobody uses”: the assertion is guarded, not the code.
#[test]
fn t422_invariant_5_names_the_offline_path() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let md = std::fs::read_to_string(root.join("CLAUDE.md")).expect("there is no CLAUDE.md");
    let i = md.find("5. **A decoder instance").expect(
        "invariant 5 was rewritten back: “all clients go through the queue” was untrue — \
         the offline path opens its own sources",
    );
    let rule = &md[i..md[i..].find("\n6. ").map(|n| i + n).unwrap_or(md.len())];
    for must in [
        "Live playback",
        "The offline path",
        "does not share",
        "promises no tempo",
        "finite and cancellable",
    ] {
        assert!(
            rule.contains(must),
            "there is no condition “{must}” in invariant 5 — the offline path is described imprecisely again"
        );
    }

    // and the conditions themselves must be feasible: the frame of a finite cancelable
    // task exists, while the index service dies together with its owner
    let bg = std::fs::read_to_string(root.join("src/apps/kt-app/src/bg.rs")).expect("there is no bg.rs");
    assert!(bg.contains("impl<P, R> Drop for BgJob"), "the task has no cancellation on Drop");
    let util =
        std::fs::read_to_string(root.join("src/apps/kt-app/src/util.rs")).expect("there is no util.rs");
    assert!(util.contains("impl Drop for IndexService"), "the index service does not die with its owner");
}
