//! # Apple Music Theme Art Daemon Library
//!
//! This library provides the core functions for finding the active Apple Music player instance,
//! resolving its album art file paths, extracting dominant colors from the cover art,
//! and updating system borders (Hyprland) and status bars (Waybar) dynamically.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use serde::Deserialize;
use image::GenericImageView;
use image::io::Reader as ImageReader;

/// Path to Waybar dynamic colors CSS file.
pub const WAYBAR_COLORS_CSS: &str = "~/.config/waybar/colors.css";
/// Fallback Hex color for the primary theme color.
pub const DEFAULT_PRIMARY: &str = "#33ccff";
/// Fallback Hex color for the accent theme color.
pub const DEFAULT_ACCENT: &str = "#00ff99";

/// Represents a window client queried from Hyprland.
#[derive(Deserialize, Debug)]
pub struct HyprlandClient {
    /// Process ID of the client window.
    pub pid: i32,
    /// Current window title.
    pub title: String,
    /// Window class identifier.
    pub class: String,
}

/// Simple RGB pixel representation for color extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Converts an RGB color to HSV representation.
///
/// Returns a tuple containing `(hue, saturation, value)` where all fields are `0.0` to `1.0`.
pub fn rgb_to_hsv(rgb: Rgb) -> (f64, f64, f64) {
    let r = rgb.r as f64 / 255.0;
    let g = rgb.g as f64 / 255.0;
    let b = rgb.b as f64 / 255.0;
    
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    
    let h = if delta == 0.0 {
        0.0
    } else if (max - r).abs() < 1e-9 {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < 1e-9 {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    
    let h = if h < 0.0 { h + 360.0 } else { h } / 360.0;
    let s = if max == 0.0 { 0.0 } else { delta / max };
    let v = max;
    
    (h, s, v)
}

/// Finds the active MPRIS player instance running Apple Music.
///
/// It scans open Hyprland clients to isolate the Apple Music window's PID,
/// and maps it back to its corresponding `playerctl` instance (`chromium.instance<PID>`).
/// If no dedicated PID match is found, it falls back to checking process command lines or
/// any active chromium media players.
pub fn find_apple_music_player() -> Option<String> {
    // 1. Get Hyprland clients JSON
    let output = Command::new("hyprctl")
        .args(&["clients", "-j"])
        .output()
        .ok()?;
    let clients: Vec<HyprlandClient> = serde_json::from_slice(&output.stdout).ok()?;
    
    let mut apple_music_pids = Vec::new();
    for client in clients {
        if client.title.contains("Apple Music") || client.class.contains("apple-music") {
            apple_music_pids.push(client.pid);
        }
    }
    
    // 2. Get playerctl players
    let output = Command::new("playerctl")
        .arg("-l")
        .output()
        .ok()?;
    let players_str = String::from_utf8_lossy(&output.stdout);
    let players: Vec<String> = players_str
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    
    // Try matching by PID first
    for pid in &apple_music_pids {
        let expected_player = format!("chromium.instance{}", pid);
        if players.contains(&expected_player) {
            return Some(expected_player);
        }
    }
    
    // Fallback 1: check command line of any chromium instances
    for player in &players {
        if player.starts_with("chromium.instance") {
            let pid_str = player.replace("chromium.instance", "");
            if let Ok(cmdline) = fs::read_to_string(format!("/proc/{}/cmdline", pid_str)) {
                if cmdline.contains("music.apple.com") || cmdline.contains("apple-music-chrome") {
                    return Some(player.clone());
                }
            }
        }
    }
    
    // Fallback 2: return any chromium player
    for player in &players {
        if player.contains("chromium") {
            return Some(player.clone());
        }
    }
    
    // Fallback 3: first active player
    if !players.is_empty() {
        return Some(players[0].clone());
    }
    
    None
}

/// Resolves a local path for the track's cover art.
///
/// If the URL starts with `http`, it downloads the cover art using `curl` to `/tmp`.
/// If it is a `file://` URL, it strips the protocol prefix to get the absolute path.
pub fn get_local_art_path(art_url: &str) -> Option<PathBuf> {
    let mut local_path = art_url.to_string();
    
    if art_url.starts_with("http") {
        let temp_path = "/tmp/apple_music_cover_art.jpg";
        let status = Command::new("curl")
            .args(&["-s", "-o", temp_path, art_url])
            .status()
            .ok()?;
        if !status.success() {
            eprintln!("Curl failed to download cover art from {}", art_url);
            return None;
        }
        local_path = temp_path.to_string();
    } else if art_url.starts_with("file://") {
        local_path = art_url.replace("file://", "");
    }
    
    let path = PathBuf::from(local_path);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Extracts a palette of 8 dominant colors from the cover art image.
///
/// It downscales the image using a fast thumbnail resize and computes a
/// color popularity histogram (quantized into bins of 16 for color grouping).
pub fn extract_colors(art_url: &str) -> Option<Vec<Rgb>> {
    let local_path = get_local_art_path(art_url)?;
    
    // Guess image format from content bytes (avoids errors with extensionless files)
    let img = match ImageReader::open(&local_path) {
        Ok(reader) => match reader.with_guessed_format() {
            Ok(guessed_reader) => match guessed_reader.decode() {
                Ok(i) => i,
                Err(e) => {
                    eprintln!("Failed to decode image at {:?}: {:?}", local_path, e);
                    return None;
                }
            },
            Err(e) => {
                eprintln!("Failed to guess image format for {:?}: {:?}", local_path, e);
                return None;
            }
        },
        Err(e) => {
            eprintln!("Failed to open file at {:?}: {:?}", local_path, e);
            return None;
        }
    };
    
    let resized = img.thumbnail(64, 64);
    
    // Color popularity histogram
    let mut histogram = HashMap::new();
    for pixel in resized.pixels() {
        let rgba = pixel.2;
        if rgba[3] < 128 {
            continue;
        }
        let quantized = (
            (rgba[0] / 16) * 16,
            (rgba[1] / 16) * 16,
            (rgba[2] / 16) * 16,
        );
        *histogram.entry(quantized).or_insert(0) += 1;
    }
    
    let mut sorted_colors: Vec<_> = histogram.into_iter().collect();
    sorted_colors.sort_by(|a, b| b.1.cmp(&a.1));
    
    let colors: Vec<Rgb> = sorted_colors
        .into_iter()
        .take(8)
        .map(|(color, _)| Rgb { r: color.0, g: color.1, b: color.2 })
        .collect();
        
    Some(colors)
}

/// Selects a Primary and Accent color pair from the extracted palette.
///
/// Colors are sorted by saturation, and candidates with mid-range brightness
/// are selected as accents to ensure high aesthetic appeal and legibility.
pub fn select_theme_colors(colors: &[Rgb]) -> (Rgb, Rgb) {
    if colors.is_empty() {
        let c1 = Rgb { r: 51, g: 204, b: 255 }; // #33ccff
        let c2 = Rgb { r: 0, g: 255, b: 153 };  // #00ff99
        return (c1, c2);
    }
    
    let mut sorted_by_sat = colors.to_vec();
    sorted_by_sat.sort_by(|a, b| {
        let sat_a = rgb_to_hsv(*a).1;
        let sat_b = rgb_to_hsv(*b).1;
        sat_b.partial_cmp(&sat_a).unwrap_or(std::cmp::Ordering::Equal)
    });
    
    let accent_candidates: Vec<Rgb> = sorted_by_sat
        .into_iter()
        .filter(|c| {
            let (_, _, v) = rgb_to_hsv(*c);
            v > 0.15 && v < 0.95
        })
        .collect();
        
    let (c1, c2) = if accent_candidates.len() >= 2 {
        (accent_candidates[0], accent_candidates[1])
    } else if accent_candidates.len() == 1 {
        let c1 = accent_candidates[0];
        let c2 = if colors[0] != c1 { colors[0] } else { colors[1] };
        (c1, c2)
    } else {
        let c1 = colors[0];
        let c2 = if colors.len() > 1 { colors[1] } else { colors[0] };
        (c1, c2)
    };
    
    let mut final_c2 = c2;
    if c1 == c2 && colors.len() > 2 {
        final_c2 = colors[2];
    }
    
    (c1, final_c2)
}

/// Applies the primary and accent colors to the system compositor and status bar.
///
/// It updates Hyprland active border parameters dynamically, updates Waybar's
/// colors stylesheet (`colors.css`), and reloads Waybar with a `USR2` signal.
pub fn update_theme(primary: Rgb, accent: Rgb) {
    let primary_hex = format!("#{:02x}{:02x}{:02x}", primary.r, primary.g, primary.b);
    let accent_hex = format!("#{:02x}{:02x}{:02x}", accent.r, accent.g, accent.b);
    println!("Applying theme: Primary={}, Accent={}", primary_hex, accent_hex);
    
    let c1_rgba = format!("rgba({:02x}{:02x}{:02x}ee)", primary.r, primary.g, primary.b);
    let c2_rgba = format!("rgba({:02x}{:02x}{:02x}ee)", accent.r, accent.g, accent.b);
    
    let _ = Command::new("hyprctl")
        .args(&["keyword", "general:col.active_border", &format!("{} {} 45deg", c1_rgba, c2_rgba)])
        .output();

    let css_path = WAYBAR_COLORS_CSS.replace('~', &std::env::var("HOME").unwrap_or_default());
    let css_content = format!(
        "/* Generated dynamically from Apple Music Cover Art */\n\
         @define-color apple-music-primary {};\n\
         @define-color apple-music-accent {};\n",
        primary_hex, accent_hex
    );
    
    if let Some(parent) = Path::new(&css_path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    
    if fs::write(&css_path, css_content).is_ok() {
        let _ = Command::new("killall")
            .args(&["-USR2", "waybar"])
            .output();
    }
}
