//! How many devices an export can afford (gpu-render-hardening, D1). Each device of a software
//! adapter keeps its own textures and compiled shaders, so the count is bounded by the memory there
//! is, not only by the cores. Speed is secondary: a render must not die of memory.

/// What a render takes whatever the device count, and what one device adds: fixed parts plus a
/// share proportional to the frame. Measured on the showcase at 1080p (gpu-render-hardening, D1).
const BASE_FIXED: u64 = 1300 << 20;
const BASE_FRAMES: u64 = 240;
const DEVICE_FIXED: u64 = 800 << 20;
const DEVICE_FRAMES: u64 = 64;

/// The threads of a video decoder of an export. Auto takes 16 on a big machine, and seven VP9 files
/// then held 2.7 GB of frames with no gain: the showcase took 138 s at four threads and 146 s at
/// auto. A render decodes a chunk at a time, not against a clock as the preview does.
pub fn decode_threads(available: Option<u64>, w: u32, h: u32) -> u32 {
    if tight(available, w, h) { 2 } else { 4 }
}

/// The threads that convert a render's decoded pictures to RGBA (render-speed-no-card D2): the
/// conversion took four times the decoding on the showcase. A tight render keeps its footprint.
pub fn rgba_threads(available: Option<u64>, w: u32, h: u32) -> u32 {
    if tight(available, w, h) { 1 } else { 4 }
}

/// The memory does not hold one device and the render's base: the render runs on its smallest
/// footprint (`ExportOptions::tight`).
pub fn tight(available: Option<u64>, w: u32, h: u32) -> bool {
    let frame = w as u64 * h as u64 * 4;
    available.is_some_and(|a| a - a / 10 < BASE_FIXED + BASE_FRAMES * frame + DEVICE_FIXED + DEVICE_FRAMES * frame)
}

/// The devices of an export of `w`×`h` on a software adapter: an eighth of the cores, 2 to 4 (past
/// four the showcase got no faster and took 6 GB more), as many of those as the memory holds, and
/// at least one. Unknown memory leaves the two a small machine would get.
pub fn engines_for(cores: usize, available: Option<u64>, w: u32, h: u32) -> usize {
    let ceiling = (cores / 8).clamp(2, 4);
    let Some(available) = available else { return ceiling.min(2) };
    let frame = w as u64 * h as u64 * 4;
    let base = BASE_FIXED + BASE_FRAMES * frame;
    let device = DEVICE_FIXED + DEVICE_FRAMES * frame;
    let spare = (available - available / 10).saturating_sub(base);
    ((spare / device) as usize).clamp(1, ceiling)
}

/// The memory this process can still take: the system's, or its control group's when that is less.
pub fn available() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
        let free = meminfo.lines().find_map(|l| l.strip_prefix("MemAvailable:"))?;
        let free: u64 = free.trim().strip_suffix("kB")?.trim().parse().ok()?;
        let mut left = free * 1024;
        // cgroup v2: every level up the path may set a limit
        let own = std::fs::read_to_string("/proc/self/cgroup").ok()?;
        let path = own.lines().find_map(|l| l.strip_prefix("0::"))?.trim_start_matches('/').to_string();
        let mut dir = std::path::PathBuf::from("/sys/fs/cgroup").join(path);
        loop {
            let read = |f: &str| std::fs::read_to_string(dir.join(f)).ok()?.trim().parse::<u64>().ok();
            if let (Some(max), Some(cur)) = (read("memory.max"), read("memory.current")) {
                left = left.min(max.saturating_sub(cur));
            }
            if dir.as_path() == std::path::Path::new("/sys/fs/cgroup") || !dir.pop() {
                break;
            }
        }
        Some(left)
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        // SAFETY: a zeroed plain struct with its length set, as the call asks
        unsafe {
            let mut m: MEMORYSTATUSEX = std::mem::zeroed();
            m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
            (GlobalMemoryStatusEx(&mut m) != 0).then_some(m.ullAvailPhys)
        }
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        None
    }
}
