//! `kt-cli fx-cost` (audio-fx-cost): what one instance of every audio effect costs against real
//! time, the node chain alone (`TrackProcessor`), at its defaults and at heavy settings, with the
//! ratio to the reference kernel the cost guard t1980 counts in (`kt_fx::audio::cost`, one text).
//! Judge the numbers on a RELEASE build; `KT_SIMD=0` runs the baseline path.

use kt_fx::audio::cost;
use kt_model::FxInstance;

pub fn run(args: &[String], project: Option<&kt_model::Project>) -> i32 {
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
    let secs: f64 = opt("--secs").and_then(|s| s.parse().ok()).unwrap_or(10.0);
    let runs: usize = opt("--runs").and_then(|s| s.parse().ok()).unwrap_or(3);
    let only = opt("--kind").cloned();
    let input = cost::stimulus((secs * 48_000.0) as usize);
    // `--project P --clip ID`: the effects of that event with its values (the person's settings)
    let clip_fx: Option<Vec<FxInstance>> = project.and_then(|p| {
        let id: u64 = opt("--clip")?.parse().ok()?;
        Some(p.clips.iter().find(|c| c.id.0 == id)?.fx.clone())
    });
    if project.is_some() && clip_fx.is_none() {
        eprintln!("--project needs --clip <id> of an event in it");
        return 2;
    }
    let kinds: Vec<&FxInstance> = match &clip_fx {
        Some(fx) => fx.iter().collect(),
        None => FxInstance::all().iter().collect(),
    };
    let kinds: Vec<&FxInstance> =
        kinds.into_iter().filter(|f| f.is_audio() && only.as_deref().is_none_or(|k| k == f.tag())).collect();
    if args.iter().any(|a| a == "--params") {
        for f in &kinds {
            let names: Vec<&str> = f.param_specs().iter().map(|s| s.name.as_ref()).collect();
            println!("{}: {}", f.tag(), names.join(" "));
        }
        return 0;
    }
    if args.iter().any(|a| a == "--digest") {
        // t1981's table: the node chain's output at the defaults and at the heavy settings
        for f in &kinds {
            let d = cost::digest(&cost::chain_output(&[(*f).clone()], &input));
            let h = cost::heavy(f).map_or(0, |h| cost::digest(&cost::chain_output(&[h], &input)));
            println!("    (\"{}\", 0x{d:016x}, 0x{h:016x}),", f.tag());
        }
        for (name, a, b) in cost::switches() {
            println!("    (\"{name}\", 0x{:016x}, 0),", cost::digest(&cost::switched_output(&a, &b, &input)));
        }
        return 0;
    }
    println!("cpu path: {}", if kt_plugin_api::cpu::fast() { "avx2" } else { "baseline" });
    if let Some(fx) = &clip_fx {
        let all: Vec<FxInstance> = fx.iter().filter(|f| f.is_audio()).cloned().collect();
        let c = cost::chain_cost(&all, &input, runs);
        println!("whole chain of the event: {:.3}% ({:.2} ref)", c.share * 100.0, c.ratio());
    }
    println!(
        "{:<20} {:>9} {:>8} {:>9} {:>8}   (share of real time per instance, x reference kernel; {secs} s, best of {runs})",
        "kind", "default", "ref", "heavy", "ref"
    );
    for f in kinds {
        let d = cost::chain_cost(std::slice::from_ref(f), &input, runs);
        let h = if clip_fx.is_some() { None } else { cost::heavy(f) }.map(|h| cost::chain_cost(&[h], &input, runs));
        let hs = h.map_or("-".to_string(), |h| format!("{:>8.3}% {:>8.2}", h.share * 100.0, h.ratio()));
        println!("{:<20} {:>8.3}% {:>8.2} {:>18}", f.tag(), d.share * 100.0, d.ratio(), hs);
    }
    0
}
