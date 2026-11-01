# CyberClip

**A terminal-first clipboard manager for Linux, built with Rust.**

CyberClip brings clipboard management directly into your terminal. Copy text, files, images, and multiple files; browse clipboard history; search and restore previous entries; and explore your filesystem through an interactive terminal interface.

No bulky desktop GUI. No need to leave your terminal.

---

## Table of Contents

- [Features](#features)
- [Screenshots](#screenshots)
- [Requirements](#requirements)
- [Installation](#installation)
- [Building from Source](#building-from-source)
- [Getting Started](#getting-started)
- [Command Reference](#command-reference)
- [TUI File Browser](#tui-file-browser)
- [Clipboard History](#clipboard-history)
- [Archive Preview](#archive-preview)
- [Examples](#examples)
- [Troubleshooting](#troubleshooting)
- [Project Structure](#project-structure)
- [Contributing](#contributing)
- [Roadmap](#roadmap)
- [License](#license)

---

## Features

### Clipboard Management

- Copy text directly from the terminal.
- Copy individual files.
- Copy multiple files in one command.
- Copy images.
- Paste clipboard contents.
- Maintain clipboard history.
- Search previous clipboard entries.
- Pin important clipboard entries.
- Restore previous clipboard entries.
- Clear clipboard history.

### File Browser

- Interactive terminal-based file browser.
- Browse directories and files.
- File-type icons.
- Preview supported files.
- Navigate your filesystem without opening a graphical file manager.

### Archive Preview

- Browse supported archive contents.
- Inspect files inside archives without manually extracting them.
- Read-only archive browsing.

### Content Extraction

- Copy the entire contents of a text file.
- Search for text inside files.
- Extract individual lines or line ranges.
- Optionally suppress line numbers.

### Built with Rust

CyberClip is written in Rust, with a focus on speed, simplicity, and maintainability.

---
### screenshots


 <h4>Codes highlighting </h4>
<p align="center">

   <img src="https://raw.githubusercontent.com/spoilpetal/Picture_Vault/refs/heads/main/CyberClip_pictures/CyberClip_code_file_preview.png">
   </img>



 <h4>A file information overview</h4>
<p align="center">

   <img src="https://raw.githubusercontent.com/spoilpetal/Picture_Vault/refs/heads/main/CyberClip_pictures/CyberClip_a_code_file_overview.png">
   </img>





 <h4>A picture preview</h4>
<p align="center">

   <img src="https://raw.githubusercontent.com/spoilpetal/Picture_Vault/refs/heads/main/CyberClip_pictures/CyberClip_a_picture_preview.png" >
   </img>



 <h4>Archive Preview</h4>
<p align="center">

   <img src="https://raw.githubusercontent.com/spoilpetal/Picture_Vault/refs/heads/main/CyberClip_pictures/CyberClip_a_archive_preview.png" >
   </img>




 <h4>CyberClip clipboard history preview</h4>

<p align="center">

   <img src="https://raw.githubusercontent.com/spoilpetal/Picture_Vault/refs/heads/main/CyberClip_pictures/CyberClip_clipboard_ui_history.png" >
   </img>

 



---


## Requirements

### Current platform

CyberClip currently targets Linux.

The current clipboard implementation uses Wayland clipboard utilities.

### Required software

- Rust
- Cargo
- Git
- `wl-clipboard`
- A working Wayland session for graphical clipboard integration

### Install dependencies

**Arch Linux**

```bash
sudo pacman -Syu
sudo pacman -S rust cargo git wl-clipboard
```

**Ubuntu / Debian**

```bash
sudo apt update
sudo apt install build-essential curl git wl-clipboard
```

Install Rust using the official Rust installation instructions:

https://www.rust-lang.org/tools/install

**Fedora**

```bash
sudo dnf install rust cargo git wl-clipboard
```

**openSUSE**

```bash
sudo zypper install rust cargo git wl-clipboard
```

Package availability and names may vary by distribution.

---

## Installation

### Clone the repository

```bash
git clone https://github.com/spoilpetal/CyberClip.git
```

Enter the project directory:

```bash
cd CyberClip
```

### Build the release version

```bash
cargo build --release
```

The compiled executable will be located at:

```text
target/release/cyberclip
```

### Run CyberClip

```bash
./target/release/cyberclip --help
```

### Optional: Install the executable locally

```bash
sudo install -Dm755 target/release/cyberclip /usr/local/bin/cyberclip
```

Now you can run:

```bash
cyberclip --help
```

---

## Building from Source

If you want to contribute or modify CyberClip, use the following workflow.

### 1. Clone the repository

```bash
git clone https://github.com/spoilpetal/CyberClip.git
cd CyberClip
```

### 2. Check the project

```bash
cargo check
```

### 3. Build a debug version

```bash
cargo build
```

### 4. Build an optimized release

```bash
cargo build --release
```

### 5. Run the program

```bash
cargo run -- --help
```

### 6. Run tests

```bash
cargo test
```

### 7. Clean build artifacts

```bash
cargo clean
```

---

# Getting Started

CyberClip provides several commands for clipboard management.

To see the available commands:

```bash
cyberclip --help
```

For command-specific help:

```bash
cyberclip copy --help
cyberclip get --help
cyberclip text --help
```

The examples below demonstrate the intended command usage.

---

# Command Reference

## 1. Copy

Copy files into your system clipboard.

### Copy one file

```bash
cyberclip copy ~/Downloads/document.pdf
```

### Copy multiple files

```bash
cyberclip copy ~/Downloads/video.mp4 ~/Downloads/image.png
```

### Copy files using a wildcard

```bash
cyberclip copy ~/Downloads/*.mp4
```

The shell expands the wildcard before CyberClip receives the arguments.

**Important:** Quote paths containing spaces.

```bash
cyberclip copy "~/Downloads/my video.mp4"
```

This preserves the file reference rather than copying the binary contents as ordinary text.

## 2. Text

Copy literal text directly into your clipboard.

```bash
cyberclip text "Hello from CyberClip!"
```

You can also copy a command as literal text:

```bash
cyberclip text "./target/release/cyberclip ~/Downloads/video.mp4"
```

**Important:** Put quotation marks around the entire text when it contains spaces. Otherwise, your shell will split it into multiple arguments.

## 3. Get

Retrieve file contents and copy the result into your clipboard.

### Copy an entire text file

```bash
cyberclip get notes.txt
```

This copies the contents without adding the filename.

### Search for text in a file

```bash
cyberclip get "CyberClip" notes.txt
```

### Extract a single line

```bash
cyberclip get line 15 notes.txt
```

### Extract a range of lines

```bash
cyberclip get line 10:25 notes.txt
```

### Suppress line numbers

```bash
cyberclip get -n line 10:25 notes.txt
```

The `get` command is useful when you want to retrieve specific information from a file without opening a text editor.

## 4. Paste

Paste clipboard contents into the terminal.

```bash
cyberclip paste
```

## 5. History

View previously copied clipboard entries.

```bash
cyberclip history
```

History entries display icons and item types to make it easier to identify text, images, files, and other content.

## 6. Search

Search through clipboard history.

```bash
cyberclip search "keyword"
```

## 7. Pin

Pin an important clipboard history entry so it is easier to find later.

```bash
cyberclip pin --help
```

Use the available syntax shown by the command help.

## 8. Remove

Remove a selected clipboard history entry.

```bash
cyberclip remove --help
```

## 9. Clear

Clear clipboard history.

```bash
cyberclip clear
```

Use this command carefully because clearing history removes saved entries.

## 10. UI

Launch the interactive terminal interface.

```bash
cyberclip ui
```

## 11. Files

Open the terminal file browser.

```bash
cyberclip files
```

---

# TUI File Browser

CyberClip includes an interactive file browser designed for people who prefer terminal workflows.

The file browser allows you to navigate directories, inspect files, and preview supported content without opening a separate graphical application.

### Highlights

- File and folder navigation.
- File-type icons.
- File previews.
- Archive browsing.
- Terminal-native workflow.

The exact keyboard shortcuts are available in the interface.

---

# Clipboard History

Clipboard history lets you revisit previously copied content.

Instead of losing something you copied earlier, you can find it in the history list.

History entries include visual icons to distinguish different types of content.

Supported entry categories include:

- Text
- Images
- Individual files
- Multiple files

You can also search, pin, restore, and remove entries.

---

# Archive Preview

CyberClip includes archive preview functionality in its terminal file browser.

The goal is to let users inspect archive contents without manually extracting everything.

Archive support depends on the formats implemented in the current release.

Archive preview is designed as a read-only browsing experience.

---

# Examples

### Copy a video

```bash
cyberclip copy ~/Videos/movie.mp4
```

### Copy several images

```bash
cyberclip copy ~/Pictures/*.png
```

### Copy a command as text

```bash
cyberclip text "sudo pacman -Syu"
```

### Copy the contents of a configuration file

```bash
cyberclip get ~/.config/hypr/hyprland.conf
```

### Extract a section from a log file

```bash
cyberclip get line 100:150 /var/log/pacman.log
```

### Open clipboard history

```bash
cyberclip history
```

### Launch the file browser

```bash
cyberclip files
```

---

# Troubleshooting

### CyberClip cannot access the clipboard

Check whether you are running a supported Wayland session.

```bash
echo $WAYLAND_DISPLAY
```

Check whether the clipboard utilities are installed:

```bash
which wl-copy
which wl-paste
```

Install `wl-clipboard` using your distribution's package manager if necessary.

### Cargo reports compilation errors

Update Rust:

```bash
rustup update
```

Check the project:

```bash
cargo check
```

If necessary, clean and rebuild:

```bash
cargo clean
cargo build --release
```

### A file path contains spaces

Put quotation marks around the path:

```bash
cyberclip copy "~/Downloads/my document.pdf"
```

### A text command contains multiple words

Quote the complete text:

```bash
cyberclip text "This is a complete sentence."
```

---

# Project Structure

The project is organized around its CLI, clipboard functionality, terminal interface, and file-management features.

```text
CyberClip/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── src/
│   ├── main.rs
│   ├── files.rs
│   ├── tui.rs
│   └── get.rs
└── target/
```

This is an illustrative overview. The actual source tree may contain additional modules.

---

# Contributing

CyberClip is an open-source project, and contributions are welcome.

You don't need to be an expert Rust developer to help.

Contributions can include:

- Bug fixes.
- Performance improvements.
- New clipboard features.
- Better file previews.
- Additional archive formats.
- TUI improvements.
- Documentation.
- Testing on other Linux distributions.
- Cross-platform support.

### Contribution workflow

1. Fork the repository.
2. Create a new branch.
3. Make your changes.
4. Run `cargo check`.
5. Run `cargo test`.
6. Build with `cargo build --release`.
7. Submit a pull request.

Please describe the changes and include reproduction steps for bug fixes.

---

# Roadmap

CyberClip is actively evolving. (It's up on the contributors)

Potential future improvements include:

- Native cross-platform clipboard backends.
- Windows support.
- macOS support.
- Additional archive formats.
- More file preview capabilities.
- Improved clipboard MIME support.
- Better installation and packaging.
- More TUI customization.

These are planned improvements, not promises of current support. (Please do by yourself cuz i have other stuffs to builds, i build this cuz i found it's missing also cuz i don't have proper machine to build and test !!!)

---

# Philosophy

CyberClip started with a simple idea:

**Clipboard management should be comfortable in the terminal.**

Not everyone wants another desktop application running in the background or another graphical interface to manage. (I don't)

CyberClip aims to provide a practical, terminal-first alternative that is fast, straightforward, and open to community contributions.

A clipboard don't have to be a rocket science
---

# License

Take Good Care Of This (But you don't have permitted to take a copy for sell)
---

**CyberClip — Your clipboard, your terminal.**
