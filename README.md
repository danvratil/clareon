<!--
SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>

SPDX-License-Identifier: GPL-3.0-or-later
-->

# Clareon

Clareon is an attempt at building a desktop AI assistant similar to Claude Desktop. Unlike Claude Desktop it should support wide range of model providers and aims to be multiplatform with actual proper Linux support.

## Goals

Those are the main goals of the project:

* Native application
* Global shortcut to start a new conversation
* Multiplatform (Linux, Windows, macOS)
* Support for large variaty of model providers (Anthropic, OpenAI, OpenRouter, LiteLLM, etc.)
* Local conversation history

## Non-goals

This project is not intended to be:

* A coding tool like Claude Code

# Technology

The core logic is written purely in Rust. The UI is built using QML and the [Kirigami](https://develop.kde.org/frameworks/kirigami/) framework. The bridge between Rust and Qt (QML) is provided by the [`cxx-qt`](https://github.com/KDAB/cxx-qt) crate with some customizations on top.

For platform integration (notifications, global shortcuts), we try to use crates that natively support all platforms.

# Development

Don't let the presence of `CMakeLists.txt` confuse you, for local development you can just use `cargo` like with any other Rust project.

The CMake build is present to make installing and packaging the application easier, since Cargo doesn't support any of that. The `CMakeLists.txt` will compile the Rust code (through [`corrosion`](https://github.com/corrosion-rs/corrosion)), generate a `.desktop` file and install it alongside the main binary and icons to appropriate locations. It is also used to enforce QML runtime dependencies, which again, makes it easier especially for packagers to ensure all the required QML modules are present on the system.

## Building on macOS

Install dependencies via Homebrew:

```bash
brew install qt cmake
```

[Kirigami](https://invent.kde.org/frameworks/kirigami) and [KItemModels](https://invent.kde.org/frameworks/kitemmodels) are runtime dependencies that must be built and installed from source, as they are not available via Homebrew for Qt6.

# License

This project is licensed under the GNU General Public License v3.0 or later. See the [LICENSE](LICENSE) file for details.
