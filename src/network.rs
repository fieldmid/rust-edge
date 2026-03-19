use std::fs;

#[derive(Clone, Debug)]
pub struct WifiTelemetry {
    pub interface: String,
    pub quality_percent: u8,
    pub signal_dbm: Option<i32>,
}

pub fn read_wifi_telemetry() -> Option<WifiTelemetry> {
    let content = fs::read_to_string("/proc/net/wireless").ok()?;
    let mut best: Option<(String, f32, Option<f32>)> = None;

    for line in content.lines().skip(2) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        let iface = parts[0].trim_end_matches(':').to_string();
        let link = parse_float(parts[2])?;
        let level = parse_float(parts[3]);

        match &best {
            Some((_, current_link, _)) if link <= *current_link => {}
            _ => {
                best = Some((iface, link, level));
            }
        }
    }

    let (interface, link, signal_dbm) = best?;
    let quality_percent = ((link / 70.0) * 100.0).round().clamp(0.0, 100.0) as u8;

    Some(WifiTelemetry {
        interface,
        quality_percent,
        signal_dbm: signal_dbm.map(|value| value.round() as i32),
    })
}

fn parse_float(raw: &str) -> Option<f32> {
    let sanitized = raw.trim_end_matches('.');
    sanitized.parse::<f32>().ok()
}
