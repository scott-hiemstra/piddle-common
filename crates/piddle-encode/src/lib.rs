//! Visual encodings and text helpers shared by the PIDdle family.
//!
//! * Color encodes CPU load, where 100% means one fully used core.
//! * Bubble area encodes memory (a process's RSS, a pod's working set).
//! * [`sanitize`] makes untrusted text (process names, pod names, labels)
//!   safe to show in a terminal or a GUI.
//!
//! No toolkit and no data source: the terminal UIs, the GUIs and the data
//! libraries (`piddle-core`, `kpiddle-core`) all depend on this crate.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub f64, pub f64, pub f64);

/// CPU percent stops: cool blue → cyan → yellow → orange → hot red.
const CPU_STOPS: [(f64, Rgb); 5] = [
    (0.0, Rgb(0.25, 0.47, 1.00)),
    (20.0, Rgb(0.13, 0.76, 0.88)),
    (50.0, Rgb(0.98, 0.82, 0.16)),
    (75.0, Rgb(0.98, 0.50, 0.12)),
    (100.0, Rgb(0.94, 0.18, 0.20)),
];

/// The color bar saturates at this CPU percentage (one full core).
pub const CPU_SCALE_MAX: f64 = 100.0;

pub fn cpu_color(cpu: f64) -> Rgb {
    let c = cpu.clamp(0.0, CPU_SCALE_MAX);
    for w in CPU_STOPS.windows(2) {
        let (a, ca) = w[0];
        let (b, cb) = w[1];
        if c <= b {
            let t = (c - a) / (b - a);
            return Rgb(lerp(ca.0, cb.0, t), lerp(ca.1, cb.1, t), lerp(ca.2, cb.2, t));
        }
    }
    CPU_STOPS[CPU_STOPS.len() - 1].1
}

/// Radius in world units for a resident set size; area grows linearly with memory.
pub fn mem_radius(rss_bytes: u64) -> f64 {
    let mb = rss_bytes as f64 / (1024.0 * 1024.0);
    MIN_RADIUS + 2.4 * mb.sqrt()
}

pub const MIN_RADIUS: f64 = 7.0;

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Blend two colors: `t = 0` gives `a`, `t = 1` gives `b`.
pub fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    Rgb(lerp(a.0, b.0, t), lerp(a.1, b.1, t), lerp(a.2, b.2, t))
}

/// Make text from other processes safe to display. Process names, command
/// lines and paths are chosen by whoever started the process, so they may
/// contain terminal escape sequences, carriage returns or bidirectional
/// overrides that could rewrite a terminal or disguise a name. Control
/// characters become `�` and bidi controls are dropped; ordinary Unicode is kept.
pub fn sanitize(s: &str) -> String {
    if !s.chars().any(|c| c.is_control() || is_bidi_control(c)) {
        return s.to_string();
    }
    s.chars().filter(|&c| !is_bidi_control(c)).map(|c| if c.is_control() { '\u{FFFD}' } else { c }).collect()
}

fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{061C}')
}

pub fn fmt_bytes(b: u64) -> String {
    let b = b as f64;
    const K: f64 = 1024.0;
    if b >= K * K * K {
        format!("{:.2} GiB", b / (K * K * K))
    } else if b >= K * K {
        format!("{:.1} MiB", b / (K * K))
    } else if b >= K {
        format!("{:.0} KiB", b / K)
    } else {
        format!("{b:.0} B")
    }
}

/// Compact byte rate for table columns: "0", "512B", "1.2K", "34M", "1.1G".
pub fn fmt_rate(bps: f64) -> String {
    const K: f64 = 1024.0;
    if bps < 1.0 {
        "0".into()
    } else if bps < K {
        format!("{bps:.0}B")
    } else if bps < K * K {
        format!("{:.1}K", bps / K)
    } else if bps < K * K * K {
        format!("{:.1}M", bps / (K * K))
    } else {
        format!("{:.1}G", bps / (K * K * K))
    }
}

pub fn fmt_cpu(cpu: f64) -> String {
    if cpu >= 10.0 { format!("{cpu:.0}%") } else { format!("{cpu:.1}%") }
}

/// Local wall-clock time "HH:MM:SS" for microseconds since the epoch.
pub fn fmt_clock(time_us: u64) -> String {
    // Type inferred from localtime_r (libc::time_t is deprecated on musl).
    let t = (time_us / 1_000_000) as _;
    // SAFETY: localtime_r writes only into `tm`.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
        return "--:--:--".into();
    }
    format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_endpoints() {
        assert_eq!(cpu_color(0.0), CPU_STOPS[0].1);
        assert_eq!(cpu_color(100.0), CPU_STOPS[4].1);
        assert_eq!(cpu_color(400.0), CPU_STOPS[4].1);
    }

    #[test]
    fn sanitize_neutralizes_escapes_and_bidi() {
        assert_eq!(sanitize("firefox"), "firefox");
        assert_eq!(sanitize("crème brûlée 🍮"), "crème brûlée 🍮");
        assert_eq!(
            sanitize("evil\x1b]2;title\x07\x1b[31mred\r\nx"),
            "evil\u{FFFD}]2;title\u{FFFD}\u{FFFD}[31mred\u{FFFD}\u{FFFD}x"
        );
        assert_eq!(sanitize("a\u{9b}31m"), "a\u{FFFD}31m"); // C1 CSI
        assert_eq!(sanitize("\u{202E}gpj.exe"), "gpj.exe");
    }

    #[test]
    fn radius_grows_with_memory() {
        assert!(mem_radius(1 << 30) > mem_radius(100 << 20));
        assert_eq!(mem_radius(0), MIN_RADIUS);
    }
}
