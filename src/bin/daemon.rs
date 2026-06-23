//! # Apple Music Theme Art Daemon (Background Loop Service)
//!
//! This service monitors the Apple Music MPRIS player instance for track changes.
//! When a song change is detected, it sleeps briefly (200ms) to allow the player
//! to finish downloading the cover art, extracts the primary and accent colors,
//! and applies them dynamically to the Hyprland active window border and Waybar colors.

use std::process::Command;
use std::thread;
use std::time::Duration;
use theme_art_daemon::{
    find_apple_music_player, extract_colors, select_theme_colors, update_theme, Rgb,
    fetch_high_res_cover_from_itunes
};

/// Poll interval for checking player state.
const INTERVAL: Duration = Duration::from_secs(2);

// Fallback default theme colors
const DEFAULT_PRIMARY: Rgb = Rgb { r: 51, g: 204, b: 255 }; // #33ccff
const DEFAULT_ACCENT: Rgb = Rgb { r: 0, g: 255, b: 153 };  // #00ff99

fn main() {
    let mut last_track_id = String::new();
    let mut last_player: Option<String> = None;
    
    println!("Apple Music Theme Art Daemon (Rust edition - background daemon) started.");
    
    // Apply defaults on startup
    update_theme(DEFAULT_PRIMARY, DEFAULT_ACCENT);
    
    loop {
        let player = find_apple_music_player();
        
        match player {
            None => {
                // If the player went offline, revert to default theme colors
                if last_player.is_some() {
                    println!("No active music player found. Reverting to default theme.");
                    update_theme(DEFAULT_PRIMARY, DEFAULT_ACCENT);
                    last_player = None;
                    last_track_id.clear();
                }
            }
            Some(ref p) => {
                last_player = Some(p.clone());
                
                // Get title, artist, and cover art url in a single query
                let output = Command::new("playerctl")
                    .args(&["-p", p, "metadata", "--format", "{{mpris:artUrl}}|{{xesam:title}}|{{xesam:artist}}"])
                    .output();
                    
                let metadata_str = if let Ok(out) = output {
                    String::from_utf8_lossy(&out.stdout).trim().to_string()
                } else {
                    String::new()
                };
                
                let parts: Vec<&str> = metadata_str.split('|').collect();
                if parts.len() >= 3 {
                    let art_url = parts[0].trim().to_string();
                    let title = parts[1].trim().to_string();
                    let artist = parts[2].trim().to_string();
                    
                    let track_id = format!("{}|{}", title, artist);
                    
                    // Track changes using Title + Artist to prevent spammed updates
                    if track_id != last_track_id {
                        last_track_id = track_id.clone();
                        println!("Track changed to: {} - {} (art URL: {})", title, artist, art_url);
                        
                        // Try fetching high resolution cover art from iTunes Search API first.
                        let mut resolved_art_url = art_url.clone();
                        if !title.is_empty() && !artist.is_empty() {
                            if let Some(high_res_url) = fetch_high_res_cover_from_itunes(&title, &artist) {
                                resolved_art_url = high_res_url;
                            }
                        }

                        if !resolved_art_url.is_empty() {
                            // Sleep briefly (200ms) to ensure file write is complete
                            thread::sleep(Duration::from_millis(200));
                            
                            if let Some(colors) = extract_colors(&resolved_art_url) {
                                let (primary, accent) = select_theme_colors(&colors);
                                update_theme(primary, accent);
                            } else {
                                eprintln!("Could not extract colors for track, using fallbacks.");
                                update_theme(DEFAULT_PRIMARY, DEFAULT_ACCENT);
                            }
                        } else {
                            update_theme(DEFAULT_PRIMARY, DEFAULT_ACCENT);
                        }
                    }
                }
            }
        }
        
        thread::sleep(INTERVAL);
    }
}
