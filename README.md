# Apple Music Theme Art Daemon & Fetcher 🎵

A modular, blazing-fast Rust suite that brings your system theme and terminal greeting to life based on the album art of the track currently playing in **Apple Music** under the **Hyprland** window compositor.

## Features ✨

* **Isolated Player Detection:** Interacts only with your Apple Music player (Google Chrome instance/PWA window PIDs) via `playerctl` and `hyprctl`. It won't get hijacked by other active media tabs (like YouTube or Twitch).
* **Dynamic Compositor Themes:** 
  - Automatically transitions your **Hyprland active window borders** to a gradient matching the cover art colors.
  - Generates custom CSS color variables for **Waybar** (`colors.css`) and reloads it instantly without interrupting your workflow.
* **Unified `fastfetch` Terminal Greeting (`music-fetch`):**
  - Replaces your `fastfetch` logo (e.g. Cinnamoroll) with the **current song's cover art** at the exact same size and padding.
  - Injects **Song, Artist, and Album** data as native modules alongside your CPU, shell, RAM, and storage specs.
  - Automatically falls back to your default `fastfetch` with the static logo when no music is playing.
* **Highly Optimized:** Written in Rust, using a fast color-popularity histogram algorithm, performing color extraction and decodes in milliseconds, consuming near 0% CPU and minimal RAM.

---

## Repository Structure 📂

```
theme-art-daemon/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs              # Core shared library (MPRIS querying, color extraction, theme updates)
    └── bin/
        ├── daemon.rs       # Background loop daemon (handles compositor and status bar updates)
        └── fetch.rs        # CLI shell startup tool (handles fastfetch configuration injection)
```

---

## Installation & Setup 🛠️

### 1. Prerequisites

Ensure you have the following packages installed:
```bash
# On Arch Linux:
sudo pacman -S playerctl fastfetch curl waybar
```

### 2. Build the Project

Clone the repository and compile the optimized binaries:
```bash
cargo build --release
```

Copy the binaries to your local path:
```bash
cp target/release/theme-art-daemon ~/.local/bin/
cp target/release/music-fetch ~/.local/bin/
```

---

## Configuration ⚙️

### Hyprland Autostart
Add the background daemon to your `~/.config/hypr/hyprland.conf`:
```ini
exec-once = ~/.local/bin/theme-art-daemon
```

### Zsh Greeting Fallback
Add this conditional block at the very top of your `~/.zshrc`:
```bash
# If music is playing, show cover art + music details + system stats.
# If not, fall back to default fastfetch.
if ! ~/.local/bin/music-fetch 2>/dev/null; then
    fastfetch
fi
```

### Waybar Stylesheet
At the very top of your `~/.config/waybar/style.css`, import the dynamic colors:
```css
@import "colors.css";
```
You can then bind any Waybar modules (like RAM/ZRAM) to these dynamic CSS variables:
```css
#custom-zram {
    color: @apple-music-accent;
}
#memory {
    color: @apple-music-primary;
}
```

---

## License 📄

MIT License. Feel free to customize and share!
