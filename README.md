# hass-light-sync

`hass-light-sync` is a program designed to capture the display's image and send the average color to a light in Home Assistant. This project is inspired by solutions like Philips Hue Sync and Ambilight.

## Installation

### Precompiled Binaries (Windows)
Download the zip archive from the Releases section on GitHub. Copy the `settings.example.json` file from the archive, rename it to `settings.json`, and edit it with your server information. After configuration, run `hass-light-sync.exe`.

### Compile it yourself
To build the project yourself, first install the Rust SDK. Clone the repository and run:

```bash
cargo build --release
```

After building, copy `hass-light-sync.exe` from the target directory to your preferred location. Also, copy the `settings.example.json` file from the root of the project to the same directory, rename it to `settings.json`, and edit it to match your server info.

