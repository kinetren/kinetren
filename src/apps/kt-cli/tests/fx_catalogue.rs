//! t978–t983 (effects-transitions-catchup, D4; capabilities `video-fx`, `transitions`, `audio-fx`):
//! the catalogue note F-FX-007 and the ranked catch-up waves keep describing the code.
//!
//! The note (`docs/features/fx/catalogue/market-catalogue-and-waves.md`) lists every built-in kind,
//! gives every market group a verdict and ranks the waves; the change design (D2) carries what each
//! wave builds. Every guard is a pure function of the texts, so it first runs on a broken copy held in
//! memory (red, naming the culprit) and then on the tree (green). Nothing is written anywhere.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use kt_model::{FxInstance, GeneratorKind, TransitionKind};

const NOTE: &str = "docs/features/fx/catalogue/market-catalogue-and-waves.md";
const DESIGN: &str = "openspec/changes/effects-transitions-catchup/design.md";

fn repo() -> PathBuf {
    // CARGO_MANIFEST_DIR = src/apps/kt-cli
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The text after `from` up to the next `to` (or the end).
fn between<'a>(text: &'a str, from: &str, to: &str) -> &'a str {
    let start = text.find(from).unwrap_or_else(|| panic!("no {from:?} in the text"));
    let rest = &text[start + from.len()..];
    &rest[..rest.find(to).unwrap_or(rest.len())]
}

/// Every `backticked` token.
fn ticks(text: &str) -> Vec<String> {
    text.split('`').skip(1).step_by(2).map(str::to_string).collect()
}

/// `word` occurs in `text` (lower case) not as part of a longer word.
fn mentions(text: &str, word: &str) -> bool {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word).any(|(i, _)| {
        !text[..i].chars().next_back().is_some_and(is_word)
            && !text[i + word.len()..].chars().next().is_some_and(is_word)
    })
}

/// Collapses the hard wraps of Markdown prose.
fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sorted(set: &BTreeSet<String>) -> String {
    set.iter().map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", ")
}

// ---- t978: the lists equal the registries -------------------------------------------------------

/// A video plugin registered through the test door (plugin-api-stage1, D8): the installed registry
/// holds it, the reference lists never do, so the note must not be asked to list it.
mod test_door {
    use kt_plugin_api::{Decl, Manifest, Media, Plugin, UiDecl, VideoClass, VideoKernel};

    pub const ID: &str = "catalogue_test_tint";

    pub struct Tint(Manifest);

    impl Tint {
        pub fn new() -> Tint {
            Tint(Manifest {
                id: ID.into(),
                media: Media::VideoFx,
                passport: vec![],
                rows: vec![],
                decl: Decl { video_class: Some(VideoClass::PointWise), builtin: false, ..Decl::default() },
                ui: UiDecl { name: "Catalogue Test Tint", category_key: "", keywords_key: "", ..UiDecl::default() },
            })
        }
    }

    impl Plugin for Tint {
        fn manifest(&self) -> &Manifest {
            &self.0
        }
        fn video(&self) -> Option<&dyn VideoKernel> {
            Some(self)
        }
    }

    impl VideoKernel for Tint {}
}

/// The process's registry: the built-in set plus the test-door plugin, installed once.
fn installed() -> &'static std::sync::Arc<kt_plugin_api::Registry> {
    kt_plugins_builtin::install_with(|reg| {
        reg.register_for_test(std::sync::Arc::new(test_door::Tint::new())).expect("the test-door plugin registers")
    })
}

/// The registries by the headings of the note's lists: the assembler's closed list
/// (`BUILTIN_IDS`, sorted by each manifest's media) plus the core-owned kinds — the arms of
/// `FxInstance::all()` that are not plugin slots, and every transition and generator (none of
/// them is a plugin on this branch). The installed registry is asked only for the media of a
/// built-in id, so a plugin from another door never becomes a row the note must carry.
fn registry() -> Vec<(&'static str, BTreeSet<String>)> {
    use kt_plugin_api::Media;
    let reg = installed();
    let (mut audio, mut layer, mut video) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    let mut transitions: BTreeSet<String> = TransitionKind::ALL.iter().map(|k| k.tag().to_string()).collect();
    let mut generators: BTreeSet<String> = GeneratorKind::all().iter().map(|k| k.tag().to_string()).collect();
    for id in kt_plugins_builtin::BUILTIN_IDS {
        let p = reg.get(id).unwrap_or_else(|| panic!("the built-in {id} is not installed"));
        let to = match p.manifest().media {
            Media::AudioFx => &mut audio,
            Media::VideoFx => &mut video,
            Media::Transition => &mut transitions,
            Media::Generator => &mut generators,
        };
        to.insert(id.to_string());
    }
    for f in FxInstance::all().iter().filter(|f| !matches!(f, FxInstance::Plugin(_))) {
        let to = if f.is_layer_mix() {
            &mut layer
        } else if f.is_audio() {
            &mut audio
        } else {
            &mut video
        };
        to.insert(f.tag().to_string());
    }
    vec![
        ("Audio effects", audio),
        ("Layer marker", layer),
        ("Video effects", video),
        ("Transitions", transitions),
        ("Generators", generators),
    ]
}

/// The five "Our catalogue" lists: (heading, count in the heading, ids).
fn catalogue_lists(note: &str) -> Vec<(String, usize, Vec<String>)> {
    let sec = between(note, "**Our catalogue, from the code**", "**What people use");
    sec.split("\n- **")
        .skip(1)
        .map(|item| {
            let (head, body) = item.split_once(":**").expect("a list heading ends with `:**`");
            let (name, count) = head.rsplit_once(" (").expect("a list heading carries its count");
            let count = count.trim_end_matches(')').parse().expect("the count of a list heading");
            (name.to_string(), count, ticks(body))
        })
        .collect()
}

fn check_lists(note: &str, reg: &[(&str, BTreeSet<String>)]) -> Vec<String> {
    let lists = catalogue_lists(note);
    let mut errs = vec![];
    for (name, want) in reg {
        let Some((_, count, ids)) = lists.iter().find(|(n, _, _)| n == name) else {
            errs.push(format!("the note has no list {name:?}"));
            continue;
        };
        let have: BTreeSet<String> = ids.iter().cloned().collect();
        if have.len() != ids.len() {
            errs.push(format!("{name}: an id is listed twice"));
        }
        let missing: BTreeSet<String> = want.difference(&have).cloned().collect();
        let extra: BTreeSet<String> = have.difference(want).cloned().collect();
        if !missing.is_empty() {
            errs.push(format!("{name}: missing {} (in the registry, not in the note)", sorted(&missing)));
        }
        if !extra.is_empty() {
            errs.push(format!("{name}: extra {} (in the note, not in the registry)", sorted(&extra)));
        }
        if *count != want.len() {
            errs.push(format!("{name}: the heading says ({count}), the registry has {}", want.len()));
        }
    }
    for (name, _, _) in &lists {
        if !reg.iter().any(|(n, _)| n == name) {
            errs.push(format!("the note's list {name:?} has no registry"));
        }
    }
    errs
}

#[test]
fn t978_the_note_lists_exactly_the_built_in_kinds() {
    let reg = registry();
    // the test-door plugin is installed and selectable, and the note is not asked to list it
    let id = test_door::ID;
    assert!(installed().get(id).is_some() && installed().is_test_only(id), "the test door did not register {id}");
    assert!(reg.iter().all(|(_, ids)| !ids.contains(id)), "a reference list holds the test-door plugin");
    let note = read(NOTE);
    let broken = [
        (note.replacen("`vhs`, `glow`, `event_grid`", "`vhs`, `event_grid`", 1), "missing `glow`"),
        (note.replacen("`event_grid`, `pan3d`.", "`event_grid`, `pan3d`, `sparkle`.", 1), "extra `sparkle`"),
        (note.replacen("**Transitions (33):**", "**Transitions (34):**", 1), "Transitions: the heading says (34)"),
    ];
    for (copy, culprit) in &broken {
        assert_ne!(copy, &note, "the broken copy for {culprit:?} did not change the note");
        let errs = check_lists(copy, &reg);
        assert!(errs.iter().any(|e| e.contains(culprit)), "the copy with {culprit:?} is not red by name: {errs:?}");
    }
    let errs = check_lists(&note, &reg);
    assert!(errs.is_empty(), "F-FX-007 does not list the built-in kinds:\n{}", errs.join("\n"));
}

// ---- t979: ids and capabilities exist -----------------------------------------------------------

/// The `F-XXX-NNN` ids in a text.
fn feature_refs(text: &str) -> BTreeSet<String> {
    let b = text.as_bytes();
    let mut out = BTreeSet::new();
    for (i, _) in text.match_indices("F-") {
        let code = b[i + 2..].iter().take_while(|c| c.is_ascii_uppercase()).count();
        let j = i + 2 + code;
        if code > 0 && b.get(j) == Some(&b'-') && b.len() >= j + 4 && b[j + 1..j + 4].iter().all(u8::is_ascii_digit) {
            out.insert(text[i..j + 4].to_string());
        }
    }
    out
}

/// The `id:` of every note under docs/features/.
fn feature_ids() -> BTreeSet<String> {
    fn walk(dir: &Path, out: &mut BTreeSet<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "md") {
                let text = std::fs::read_to_string(&p).unwrap();
                if let Some(id) = text.lines().find_map(|l| l.strip_prefix("id: ")) {
                    out.insert(id.trim().to_string());
                }
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(&repo().join("docs/features"), &mut out);
    out
}

/// The capabilities named by `openspec/specs/<cap>/` paths.
fn spec_paths(text: &str) -> BTreeSet<String> {
    text.match_indices("openspec/specs/")
        .filter_map(|(i, m)| text[i + m.len()..].split_once('/').map(|(cap, _)| cap.to_string()))
        .filter(|cap| !cap.is_empty() && cap.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
        .collect()
}

fn check_refs(note: &str, design: &str) -> Vec<String> {
    let known = feature_ids();
    let waves = between(design, "### D2.", "### D3.");
    let mut errs = vec![];
    let ids = feature_refs(between(note, "## Verdict", "\n## ")).into_iter().chain(feature_refs(waves));
    for id in ids.collect::<BTreeSet<_>>() {
        if !known.contains(&id) {
            errs.push(format!("{id} is not the id: of any note under docs/features/"));
        }
    }
    for cap in spec_paths(note).into_iter().chain(spec_paths(design)).collect::<BTreeSet<_>>() {
        if !repo().join("openspec/specs").join(&cap).join("spec.md").is_file() {
            errs.push(format!("openspec/specs/{cap}/ has no spec.md"));
        }
    }
    errs
}

#[test]
fn t979_the_waves_name_only_existing_notes_and_capabilities() {
    let (note, design) = (read(NOTE), read(DESIGN));
    let invented = note.replacen("Depends on F-FX-002,\n   F-FX-001.", "Depends on F-FX-002,\n   F-FX-099.", 1);
    assert_ne!(invented, note);
    let errs = check_refs(&invented, &design);
    assert!(errs.iter().any(|e| e.starts_with("F-FX-099 ")), "the invented id is not named: {errs:?}");
    let in_wave = design.replacen("depends on F-FX-002, F-FX-001", "depends on F-FX-002, F-FX-098", 1);
    assert_ne!(in_wave, design);
    let errs = check_refs(&note, &in_wave);
    assert!(errs.iter().any(|e| e.starts_with("F-FX-098 ")), "an invented id in a wave is not named: {errs:?}");
    let no_cap = format!("{design}\nSee `openspec/specs/no-such-cap/`.\n");
    let errs = check_refs(&note, &no_cap);
    assert!(errs.iter().any(|e| e.contains("no-such-cap")), "a missing capability is not named: {errs:?}");
    let errs = check_refs(&note, &design);
    assert!(errs.is_empty(), "F-FX-007 / the waves name what does not exist:\n{}", errs.join("\n"));
}

// ---- t980: verdict vocabulary and [NOT VERIFIED] ------------------------------------------------

/// The data rows of the Markdown table(s) in `sec` (header and separator dropped).
fn table_rows(sec: &str) -> Vec<Vec<String>> {
    let mut rows = vec![];
    let mut header = true;
    for l in sec.lines() {
        let l = l.trim();
        if !l.starts_with('|') {
            header = true;
            continue;
        }
        if header || l.starts_with("|---") {
            header = false;
            continue;
        }
        rows.push(l.trim_matches('|').split('|').map(|c| c.trim().to_string()).collect());
    }
    rows
}

fn market_rows(note: &str) -> Vec<Vec<String>> {
    table_rows(between(note, "**What people use, against what we have.**", "**The owner's FX items"))
}

fn owner_rows(note: &str) -> Vec<Vec<String>> {
    table_rows(between(note, "**The owner's FX items", "\n## "))
}

/// The ranked list of the Verdict: (wave number, its text), in rank order.
fn note_waves(note: &str) -> Vec<(u32, String)> {
    let sec = between(note, "## Verdict", "\n## ");
    let mut out: Vec<(u32, String)> = vec![];
    for l in sec.lines() {
        let item = l.split_once(". **W").filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
        if let Some((_, rest)) = item {
            let n = rest.split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap();
            out.push((n, rest.to_string()));
        } else if l.starts_with("   ") && !out.is_empty() {
            let last = out.last_mut().unwrap();
            last.1.push('\n');
            last.1.push_str(l);
        }
    }
    out
}

/// The numbered sources: (number, opened live = carries no `[NOT VERIFIED]`).
fn sources(note: &str) -> Vec<(u32, bool)> {
    let sec = between(note, "## Sources", "\n## ");
    let mut out: Vec<(u32, bool)> = vec![];
    for l in sec.lines() {
        match l.split_once(". ").and_then(|(n, _)| n.parse().ok()) {
            Some(n) => out.push((n, true)),
            None if l.starts_with("   ") => {}
            None => continue,
        }
        if l.contains("[NOT VERIFIED]") {
            out.last_mut().unwrap().1 = false;
        }
    }
    out
}

/// The text inside the parenthesis that opens right after `at` (nesting counted).
fn paren_body(text: &str, at: usize) -> &str {
    let mut depth = 0;
    for (i, c) in text[at..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return &text[at + 1..at + i];
                }
            }
            _ => {}
        }
    }
    &text[at + 1..]
}

fn check_verdict(cell: &str, waves: &[u32]) -> Result<(), String> {
    if !["have", "wave W", "deferred (", "rejected ("].iter().any(|v| cell.starts_with(v)) {
        return Err(format!("the verdict {cell:?} is not in the vocabulary"));
    }
    for (i, m) in cell.match_indices("wave W") {
        let n: String = cell[i + m.len()..].chars().take_while(char::is_ascii_digit).collect();
        if !n.parse().is_ok_and(|n: u32| waves.contains(&n)) {
            return Err(format!("the verdict names wave W{n}, which the Verdict list does not have"));
        }
    }
    for word in ["deferred (", "rejected ("] {
        for (i, _) in cell.match_indices(word) {
            if paren_body(cell, i + word.len() - 1).trim().is_empty() {
                return Err(format!("`{}` without a reason or dependency", word.trim_end_matches(" (")));
            }
        }
    }
    Ok(())
}

fn check_table(note: &str) -> Vec<String> {
    let waves: Vec<u32> = note_waves(note).iter().map(|(n, _)| *n).collect();
    let srcs = sources(note);
    let mut errs = vec![];
    for row in market_rows(note) {
        let [group, market, _, verdict] = row.as_slice() else {
            errs.push(format!("the row {row:?} does not have four cells"));
            continue;
        };
        if let Err(e) = check_verdict(verdict, &waves) {
            errs.push(format!("{group}: {e}"));
        }
        let mut live = false;
        for (i, m) in market.match_indices("(src ") {
            let n: String = market[i + m.len()..].chars().take_while(char::is_ascii_digit).collect();
            match srcs.iter().find(|(k, _)| k.to_string() == n) {
                Some((_, true)) => live = true,
                Some((_, false)) => errs.push(format!("{group}: cites src {n}, which was not opened live")),
                None => errs.push(format!("{group}: cites src {n}, which the Sources list does not have")),
            }
        }
        if !live && !market.contains("[NOT VERIFIED]") {
            errs.push(format!("{group}: the market cell cites no live source and carries no [NOT VERIFIED]"));
        }
    }
    for row in owner_rows(note) {
        if let Err(e) = check_verdict(row.last().unwrap(), &waves) {
            errs.push(format!("owner's item {}: {e}", row[0]));
        }
    }
    errs
}

/// The note with the last cell of the market row starting with `group` replaced.
fn with_verdict(note: &str, group: &str, verdict: &str) -> String {
    let start = note.find(&format!("\n| {group}")).unwrap_or_else(|| panic!("no row {group:?}")) + 1;
    let end = start + note[start..].find('\n').unwrap();
    let row = note[start..end].trim_end_matches('|').trim_end();
    let cut = row.rfind('|').unwrap();
    format!("{}{}| {verdict} |{}", &note[..start], &row[..cut], &note[end..])
}

#[test]
fn t980_every_catalogue_row_has_a_verdict_from_the_vocabulary() {
    let note = read(NOTE);
    let masks_unchecked = {
        let row = note.lines().find(|l| l.starts_with("| Masks")).unwrap();
        note.replacen(row, &row.replace(" `[NOT VERIFIED]`", ""), 1)
    };
    let broken = [
        (with_verdict(&note, "Masks", "maybe later"), "Masks: the verdict \"maybe later\""),
        (with_verdict(&note, "Bump mapping", "wave W9"), "Bump mapping / relief light: the verdict names wave W9"),
        (with_verdict(&note, "AI effects", "rejected ( )"), "AI effects (portrait, sky replacement, style transfer): `rejected`"),
        (with_verdict(&note, "Parallax", "deferred ()"), "Parallax / 2.5D: `deferred`"),
        (masks_unchecked, "Masks: the market cell cites no live source"),
        (note.replacen("camera shake presets (src 1)", "camera shake presets (src 2)", 1), "Shake / zoom punch: cites src 2"),
    ];
    for (copy, culprit) in &broken {
        assert_ne!(copy, &note, "the broken copy for {culprit:?} did not change the note");
        let errs = check_table(copy);
        assert!(errs.iter().any(|e| e.starts_with(culprit)), "the copy with {culprit:?} is not red by name: {errs:?}");
    }
    let errs = check_table(&note);
    assert!(errs.is_empty(), "F-FX-007's verdict table is not honest:\n{}", errs.join("\n"));
}

// ---- t981: reviving a rejected idea is an explicit revision -------------------------------------

/// The frozen list (the t643 technique): parsing design prose for "what was rejected" is fragile, and
/// a reviewer sees this list change in a diff. (name, the design that rejected it, the words in its
/// "Rejected and why", the words that mark it in a wave).
const REJECTED: &[(&str, &str, &str, &[&str])] = &[
    ("a directional blur", "video-fx", "directional (linear) blur", &["directional"]),
    ("sepia/invert as kinds", "video-fx", "sepia/invert as separate kinds", &["sepia", "invert"]),
    ("a node graph", "video-fx", "node graph", &["node graph"]),
    ("a transition progress curve", "transitions", "transition progress curve", &["progress curve"]),
    ("stylised audio transitions", "transitions", "stylised audio transitions", &["stylised audio", "stylized audio"]),
    ("a gallery of live transition previews", "transitions", "gallery of kinds with live mini-previews", &["gallery"]),
];

/// The waves of D2: (wave number, its block), in rank order.
fn design_waves(design: &str) -> Vec<(u32, String)> {
    between(design, "### D2.", "### D3.")
        .split("\n**W")
        .skip(1)
        .map(|b| (b.split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap(), b.to_string()))
        .collect()
}

/// The `Revision: …` sentences of a block (each up to the end of its bullet), flattened.
fn revisions(block: &str) -> Vec<String> {
    block
        .match_indices("Revision:")
        .map(|(i, _)| {
            let rest = &block[i..];
            flat(&rest[..rest.find("\n- ").unwrap_or(rest.len())])
        })
        .collect()
}

fn check_revisions(note: &str, design: &str) -> Vec<String> {
    let mut errs = vec![];
    for (name, cap, evidence, _) in REJECTED {
        let rejected = flat(&between(&read(&format!("openspec/specs/{cap}/design.md")), "## Rejected and why", "\n## ").to_lowercase());
        if !rejected.contains(evidence) {
            errs.push(format!("{name}: the {cap} design's \"Rejected and why\" no longer says {evidence:?}"));
        }
    }
    let notes = note_waves(note);
    for (n, block) in design_waves(design) {
        let in_note = notes.iter().find(|(k, _)| *k == n).map(|(_, t)| t.to_lowercase()).unwrap_or_default();
        let revs = revisions(&block);
        for r in &revs {
            let ok = r.strip_prefix("Revision:").and_then(|t| t.split_once(" — ")).is_some_and(|(d, why)| {
                !d.trim().is_empty() && !why.trim().is_empty()
            });
            if !ok {
                errs.push(format!("W{n}: {r:?} is not `Revision: <decision> — <reason>`"));
            }
        }
        let lower = block.to_lowercase();
        for (name, cap, _, words) in REJECTED {
            if !words.iter().any(|w| mentions(&lower, w) || mentions(&in_note, w)) {
                continue;
            }
            if !revs.iter().any(|r| words.iter().any(|w| mentions(&r.to_lowercase(), w))) {
                errs.push(format!("W{n} names {name} (rejected in {cap}) without a `Revision: … — …` line about it"));
            }
        }
    }
    errs
}

#[test]
fn t981_reviving_a_rejected_idea_is_an_explicit_revision() {
    let (note, design) = (read(NOTE), read(DESIGN));
    let start = design.find("Revision: a directional blur").expect("W2's directional revision");
    let end = start + design[start..].find("kind stands.").unwrap() + "kind stands.".len();
    let lost = format!("{}{}", &design[..start], &design[end..]);
    let errs = check_revisions(&note, &lost);
    assert!(errs.iter().any(|e| e.starts_with("W2 names a directional blur")), "the lost revision is not named: {errs:?}");
    let curve = design.replacen("`push`/`slide` whose", "`push`/`slide` with a progress curve, whose", 1);
    assert_ne!(curve, design);
    let errs = check_revisions(&note, &curve);
    assert!(errs.iter().any(|e| e.starts_with("W5 names a transition progress curve")), "{errs:?}");
    let no_reason = design.replacen("not as a kind — the video-fx", "not as a kind, the video-fx", 1);
    assert_ne!(no_reason, design);
    let errs = check_revisions(&note, &no_reason);
    assert!(errs.iter().any(|e| e.starts_with("W2: ") && e.contains("is not `Revision:")), "{errs:?}");
    let errs = check_revisions(&note, &design);
    assert!(errs.is_empty(), "a wave revives a rejected idea silently:\n{}", errs.join("\n"));
}

// ---- t982: wave order, families, capability per wave --------------------------------------------

/// The family crates a wave may name: the directories under src/plugins/ and the families the
/// plugin API's design table plans (`src/plugins/<name>` at the start of a line of any design.md).
fn known_families() -> BTreeSet<String> {
    fn walk(dir: &Path, out: &mut BTreeSet<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.file_name().is_some_and(|n| n == "design.md") {
                for l in std::fs::read_to_string(&p).unwrap().lines() {
                    if let Some(rest) = l.strip_prefix("src/plugins/") {
                        out.insert(rest.split_whitespace().next().unwrap_or("").to_string());
                    }
                }
            }
        }
    }
    let mut out: BTreeSet<String> = std::fs::read_dir(repo().join("src/plugins"))
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    walk(&repo().join("openspec"), &mut out);
    out
}

/// A capability exists, or an open change adds it (the `plugins` capability of plugin-api-stage1).
fn capability_exists(cap: &str) -> bool {
    repo().join("openspec/specs").join(cap).join("spec.md").is_file()
        || std::fs::read_dir(repo().join("openspec/changes"))
            .unwrap()
            .flatten()
            .any(|c| c.path().join("specs").join(cap).join("spec.md").is_file())
}

fn check_waves(note: &str, design: &str, audio: &BTreeSet<String>) -> Vec<String> {
    let mut errs = vec![];
    let waves = design_waves(design);
    let rank: Vec<u32> = waves.iter().map(|(n, _)| *n).collect();
    let note_rank: Vec<u32> = note_waves(note).iter().map(|(n, _)| *n).collect();
    if rank != note_rank {
        errs.push(format!("the design ranks the waves {rank:?}, the note's Verdict {note_rank:?}"));
    }
    let families = known_families();
    for (pos, (n, block)) in waves.iter().enumerate() {
        let header = &block[..block.find("):").unwrap_or(block.len())];
        // every mention of another wave is a dependency on it
        for (i, _) in block.match_indices('W') {
            let k: String = block[i + 1..].chars().take_while(char::is_ascii_digit).collect();
            let after = block[i + 1 + k.len()..].chars().next();
            let before = block[..i].chars().next_back();
            if before.is_some_and(char::is_alphanumeric) || after.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let Ok(k) = k.parse::<u32>() else { continue };
            if k == *n {
                continue;
            }
            match rank.iter().position(|r| *r == k) {
                Some(p) if p < pos => {}
                Some(_) => errs.push(format!("W{n} depends on W{k}, which is ranked after it")),
                None => errs.push(format!("W{n} names W{k}, which is not a wave")),
            }
        }
        let paths: Vec<String> = ticks(header).into_iter().filter(|t| t.starts_with("src/")).collect();
        if paths.is_empty() && !header.contains("no new kernel") {
            errs.push(format!("W{n} names no family crate under src/plugins/ and does not say \"no new kernel\""));
        }
        for p in &paths {
            match p.strip_prefix("src/plugins/") {
                Some(f) if families.contains(f) => {}
                _ => errs.push(format!("W{n}: `{p}` is not a family crate (src/plugins/ or the plugin API's family table)")),
            }
        }
        let caps: Vec<String> = ticks(header).into_iter().filter(|t| !t.starts_with("src/")).collect();
        if caps.is_empty() {
            errs.push(format!("W{n} names no capability"));
        }
        for c in caps.iter().filter(|c| !capability_exists(c)) {
            errs.push(format!("W{n}: capability `{c}` has no spec (openspec/specs/ or an open change)"));
        }
        if caps.iter().any(|c| c == "audio-fx") && !header.contains("no new kernel") {
            // the audio wave: presets chain existing ids, the new nodes are named as such
            let ids: BTreeSet<String> = ticks(block)
                .into_iter()
                .filter(|t| t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
                .collect();
            let declared: BTreeSet<String> = block
                .split("\n- ")
                .filter(|b| b.contains("new node"))
                .flat_map(ticks)
                .filter(|t| ids.contains(t) && !audio.contains(t))
                .collect();
            let unknown: BTreeSet<String> = ids.iter().filter(|t| !audio.contains(*t) && !declared.contains(*t)).cloned().collect();
            if !unknown.is_empty() {
                errs.push(format!("W{n}: {} is neither an audio id nor a declared new node", sorted(&unknown)));
            }
            if declared.is_empty() && !block.contains("no new node") {
                errs.push(format!("W{n} does not say which nodes it adds"));
            }
        }
    }
    errs
}

#[test]
fn t982_waves_are_ordered_and_name_families_and_capabilities() {
    let audio = registry().into_iter().find(|(n, _)| *n == "Audio effects").unwrap().1;
    let (note, design) = (read(NOTE), read(DESIGN));
    let swapped = note.replacen("3. **W5", "3. **W3", 1).replacen("4. **W3", "4. **W5", 1);
    let forward = design.replacen("pairs with `lens_distort`", "pairs with W6's echo and `lens_distort`", 1);
    let nowhere = design.replacen("`src/plugins/fx-video-blur`", "`src/plugins/fx-video-nowhere`", 1);
    let no_cap = design.replacen("(`src/plugins/generators-title`; capability `generators`;", "(`src/plugins/generators-title`;", 1);
    let vocoder = design.replacen("chipmunk / deep (`pitch_shift`)", "chipmunk / deep (`pitch_shift`, `vocoder`)", 1);
    let broken = [
        (&swapped, &design, "the design ranks the waves"),
        (&note, &forward, "W1 depends on W6"),
        (&note, &nowhere, "W2: `src/plugins/fx-video-nowhere`"),
        (&note, &no_cap, "W3 names no capability"),
        (&note, &vocoder, "W7: `vocoder`"),
    ];
    for (n, d, culprit) in broken {
        assert!(n != &note || d != &design, "the broken copy for {culprit:?} did not change anything");
        let errs = check_waves(n, d, &audio);
        assert!(errs.iter().any(|e| e.starts_with(culprit)), "the copy with {culprit:?} is not red by name: {errs:?}");
    }
    let errs = check_waves(&note, &design, &audio);
    assert!(errs.is_empty(), "the waves are not well-formed:\n{}", errs.join("\n"));
}

// ---- t983: the owner's FX items -----------------------------------------------------------------

/// The FX items of the owner's claims file `ktprompts_v26.10` (frozen here, as in t981): (the item,
/// the words its row starts with or carries in its first cell, lower case).
const OWNER_ITEMS: &[(&str, &str)] = &[
    ("fisheye lens corners", "fisheye lens corners"),
    ("dirt/drops/flares on the lens", "dirt / drops / flares"),
    ("letters as physical objects", "letters as physical objects"),
    ("inertia of letters under rate", "inertia of letters"),
    ("the \"67 niche plugins\" reference", "67 niche plugins"),
    ("jag edit by time remap", "jag edit"),
    ("parallax", "parallax"),
    ("text blending and shapes holding sources", "text blending"),
    ("homothety", "homothety"),
    ("bump mapping", "bump mapping"),
    ("buffer + blend levels", "blend levels"),
    ("audio-driven zoom", "zoom driven by the audio"),
    ("blur on one word of a ticker", "one word of a ticker"),
    ("fisheye switched on by speed changes", "fisheye switched on by speed"),
];

fn check_owner_items(note: &str) -> Vec<String> {
    let waves: Vec<u32> = note_waves(note).iter().map(|(n, _)| *n).collect();
    let rows: Vec<Vec<String>> = owner_rows(note).into_iter().chain(market_rows(note)).collect();
    let mut errs = vec![];
    for (item, needle) in OWNER_ITEMS {
        match rows.iter().find(|r| r[0].to_lowercase().contains(needle)) {
            None => errs.push(format!("the owner's item {item:?} has no row in F-FX-007")),
            Some(r) => {
                if let Err(e) = check_verdict(r.last().unwrap(), &waves) {
                    errs.push(format!("the owner's item {item:?}: {e}"));
                }
            }
        }
    }
    errs
}

#[test]
fn t983_every_owner_fx_item_has_a_row() {
    let note = read(NOTE);
    let row = note.lines().find(|l| l.starts_with("| Blur on one word of a ticker")).unwrap();
    let removed = note.replacen(&format!("{row}\n"), "", 1);
    let errs = check_owner_items(&removed);
    assert!(
        errs.iter().any(|e| e.contains("\"blur on one word of a ticker\" has no row")),
        "the removed item is not named: {errs:?}"
    );
    let errs = check_owner_items(&with_verdict(&note, "Jag edit", "soon"));
    assert!(errs.iter().any(|e| e.contains("\"jag edit by time remap\": the verdict")), "{errs:?}");
    let errs = check_owner_items(&note);
    assert!(errs.is_empty(), "an owner's FX item lacks a row:\n{}", errs.join("\n"));
}
