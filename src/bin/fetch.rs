use std::process::{Command, exit};
use std::fs;
use std::path::Path;
use theme_art_daemon::{find_apple_music_player, get_local_art_path, fetch_high_res_cover_from_itunes};

/// Strips C-style single-line comments from JSONC content to parse it cleanly.
fn load_jsonc(path: &Path) -> Option<serde_json::Value> {
    let content = fs::read_to_string(path).ok()?;
    let mut clean_content = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        clean_content.push_str(line);
        clean_content.push('\n');
    }
    serde_json::from_str(&clean_content).ok()
}

fn main() {
    let player = match find_apple_music_player() {
        Some(p) => p,
        None => exit(1),
    };
    
    // Check if the player is currently playing
    let status_output = Command::new("playerctl")
        .args(&["-p", &player, "status"])
        .output();
        
    let status = match status_output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(_) => exit(1),
    };
    
    if status != "Playing" {
        exit(1);
    }
    
    // Get metadata
    let metadata_output = Command::new("playerctl")
        .args(&["-p", &player, "metadata", "--format", "{{mpris:artUrl}}|{{xesam:title}}|{{xesam:artist}}|{{xesam:album}}"])
        .output();
        
    let metadata_str = match metadata_output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(_) => exit(1),
    };
    
    let parts: Vec<&str> = metadata_str.split('|').collect();
    if parts.len() < 4 {
        exit(1);
    }
    
    let art_url = parts[0].trim().to_string();
    let title = parts[1].trim().to_string();
    let artist = parts[2].trim().to_string();
    let album = parts[3].trim().to_string();
    
    // Find local cover art path. Try fetching high resolution cover art from iTunes Search API first.
    let mut local_art_path = None;
    if !title.is_empty() && !artist.is_empty() {
        if let Some(high_res_url) = fetch_high_res_cover_from_itunes(&title, &artist) {
            local_art_path = get_local_art_path(&high_res_url);
        }
    }
    
    if local_art_path.is_none() && !art_url.is_empty() {
        local_art_path = get_local_art_path(&art_url);
    }
    
    // Read original fastfetch config template
    let home = std::env::var("HOME").unwrap_or_default();
    let config_path = Path::new(&home).join(".config/fastfetch/config.jsonc");
    
    if !config_path.exists() {
        eprintln!("Error: config.jsonc does not exist");
        exit(1);
    }
    
    let mut config_val = match load_jsonc(&config_path) {
        Some(v) => v,
        None => {
            eprintln!("Error parsing config.jsonc");
            exit(1);
        }
    };
    
    // 1. Replace logo source with cover art if available
    if let Some(art_path) = local_art_path {
        if let Some(logo) = config_val.get_mut("logo") {
            if let Some(source) = logo.get_mut("source") {
                *source = serde_json::Value::String(art_path.to_string_lossy().to_string());
            }
        }
    }
    
    // 2. Inject custom song modules right after the title module using Nerd Font icons
    if let Some(modules) = config_val.get_mut("modules") {
        if let Some(modules_arr) = modules.as_array_mut() {
            let mut insert_idx = 0;
            for (i, val) in modules_arr.iter().enumerate() {
                if let Some(obj) = val.as_object() {
                    if obj.get("type").and_then(|t| t.as_str()) == Some("title") {
                        insert_idx = i + 1;
                        break;
                    }
                }
            }
            
            let mut song_modules = vec![
                serde_json::json!("break"),
                serde_json::json!({
                    "type": "custom",
                    "key": "󰎆 Song",
                    "keyColor": "blue",
                    "format": title
                }),
                serde_json::json!({
                    "type": "custom",
                    "key": " Artist",
                    "keyColor": "blue",
                    "format": artist
                }),
            ];
            
            if !album.is_empty() {
                song_modules.push(serde_json::json!({
                    "type": "custom",
                    "key": "󰀥 Album",
                    "keyColor": "blue",
                    "format": album
                }));
            }
            
            song_modules.push(serde_json::json!("break"));
            
            for (offset, item) in song_modules.into_iter().enumerate() {
                modules_arr.insert(insert_idx + offset, item);
            }
        }
    }
    
    // 3. Write temp config file
    let temp_config_path = "/tmp/fastfetch_music.json";
    let temp_config_str = match serde_json::to_string(&config_val) {
        Ok(s) => s,
        Err(_) => exit(1),
    };
    
    if fs::write(temp_config_path, temp_config_str).is_err() {
        exit(1);
    }
    
    // 4. Run fastfetch with the temp config
    let status = Command::new("fastfetch")
        .args(&["-c", temp_config_path])
        .status();
        
    match status {
        Ok(s) if s.success() => exit(0),
        _ => exit(1),
    }
}
