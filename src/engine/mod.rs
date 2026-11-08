//! FFmpeg detection, media probing and parallel proxy encoding.

pub mod batch;
pub mod caps;
pub mod command;
pub mod media;
pub mod plan;
pub mod process;

/// Video file extensions ProxyCraft accepts (lower case, with the dot).
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    ".mp4", ".mov", ".m4v", ".mkv", ".avi", ".mxf", ".mts", ".m2ts", ".ts", ".webm", ".mpg",
    ".mpeg", ".3gp", ".wmv", ".flv", ".dv",
];

pub const DEFAULT_SUFFIX: &str = "_Proxy";
pub const PROXIES_FOLDER: &str = "Proxies";

/// Where proxies are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Destination {
    /// `<source folder>/Proxies/`
    #[serde(rename = "proxies_folder")]
    ProxiesFolder,
    /// Next to the source file.
    #[serde(rename = "same_folder")]
    SameFolder,
    /// One folder for the whole batch.
    #[serde(rename = "custom")]
    Custom,
}

impl Destination {
    pub const ALL: [Destination; 3] = [
        Destination::ProxiesFolder,
        Destination::SameFolder,
        Destination::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Destination::ProxiesFolder => "Next to the original media, in a Proxies folder",
            Destination::SameFolder => "Next to the original media, same folder",
            Destination::Custom => "One folder for all proxies",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Destination::ProxiesFolder => "proxies_folder",
            Destination::SameFolder => "same_folder",
            Destination::Custom => "custom",
        }
    }

    pub fn from_key(key: &str) -> Option<Destination> {
        Destination::ALL.into_iter().find(|d| d.key() == key)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    ProRes,
    H264,
    H265,
}

impl Family {
    pub fn name(self) -> &'static str {
        match self {
            Family::ProRes => "ProRes",
            Family::H264 => "H.264",
            Family::H265 => "H.265",
        }
    }
}

/// An output format and size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    pub key: &'static str,
    pub label: &'static str,
    pub family: Family,
    /// 2 = half size, 4 = quarter size.
    pub divisor: u32,
    /// ProRes profile number.
    pub profile: &'static str,
}

impl Preset {
    pub fn extension(&self) -> &'static str {
        if self.family == Family::ProRes {
            ".mov"
        } else {
            ".mp4"
        }
    }

    pub fn container(&self) -> &'static str {
        if self.family == Family::ProRes {
            "mov"
        } else {
            "mp4"
        }
    }
}

pub const PRESETS: [Preset; 7] = [
    Preset {
        key: "prores_proxy_quarter",
        label: "ProRes 422 Proxy, ¼ size",
        family: Family::ProRes,
        divisor: 4,
        profile: "0",
    },
    Preset {
        key: "prores_proxy_half",
        label: "ProRes 422 Proxy, ½ size",
        family: Family::ProRes,
        divisor: 2,
        profile: "0",
    },
    Preset {
        key: "prores_lt_half",
        label: "ProRes 422 LT, ½ size",
        family: Family::ProRes,
        divisor: 2,
        profile: "1",
    },
    Preset {
        key: "h264_quarter",
        label: "H.264, ¼ size",
        family: Family::H264,
        divisor: 4,
        profile: "",
    },
    Preset {
        key: "h264_half",
        label: "H.264, ½ size",
        family: Family::H264,
        divisor: 2,
        profile: "",
    },
    Preset {
        key: "h265_quarter",
        label: "H.265, ¼ size",
        family: Family::H265,
        divisor: 4,
        profile: "",
    },
    Preset {
        key: "h265_half",
        label: "H.265, ½ size",
        family: Family::H265,
        divisor: 2,
        profile: "",
    },
];

pub fn preset_by_key(key: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.key == key)
}

/// "HH:MM:SS", rounding to the nearest second.
pub fn format_time(seconds: f64) -> String {
    let s = if seconds.is_finite() {
        seconds.max(0.0).round() as u64
    } else {
        0
    };
    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}

/// Drop FFmpeg's "[mov,mp4 @ 0x…]" prefixes so messages read plainly.
pub fn clean_error(text: &str) -> String {
    text.lines()
        .map(|l| {
            let t = l.trim();
            if t.starts_with('[') {
                if let Some(end) = t.find(']') {
                    let inside = &t[1..end];
                    if let Some(at) = inside.rfind('@') {
                        let addr = inside[at + 1..].trim();
                        if !addr.is_empty()
                            && addr
                                .chars()
                                .all(|c| c.is_ascii_hexdigit() || c == 'x' || c == 'X')
                        {
                            return t[end + 1..].trim().to_string();
                        }
                    }
                }
            }
            t.to_string()
        })
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_and_errors() {
        assert_eq!(format_time(3725.4), "01:02:05");
        assert_eq!(format_time(-3.0), "00:00:00");
        assert_eq!(
            clean_error("[mov,mp4,m4a @ 0x55d1] moov atom not found\n\n  x  "),
            "moov atom not found\nx"
        );
        assert_eq!(clean_error("[plain] kept"), "[plain] kept");
    }

    #[test]
    fn presets() {
        assert_eq!(PRESETS.len(), 7);
        assert_eq!(preset_by_key("prores_lt_half").unwrap().profile, "1");
        assert_eq!(preset_by_key("h265_half").unwrap().extension(), ".mp4");
        assert_eq!(Destination::from_key("custom"), Some(Destination::Custom));
    }
}
