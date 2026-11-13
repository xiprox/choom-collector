# 002: The window is drawn with gpui, through Longbridge's gpui-kit

The app's window is drawn with gpui, Zed's GPU-accelerated UI framework, and its controls come from Longbridge's gpui-component, through the `gpui-kit` crate that bundles both.
Decided by the user on 2026-11-13, after a spike that built the controls the app needs.

## Context

The window needs a table of mods with checkboxes, tooltips, text inputs, and state that background threads change every second, on Windows and on Linux, the Steam Deck included.
The user wanted a desktop app that feels like a professional tool, and ruled out a web view: "yes, no tauri" (user, 2026-11-13).

## Options

1. **egui.** Quick to build with and small, but it looks like a developer tool until themed.
2. **iced.** Clean and well maintained, with fewer ready-made widgets and weaker accessibility.
3. **Slint.** The most polished look out of the box, with its UI in a markup language of its own and a credit or GPL license.
4. **gpui, with gpui-component.** Zed's look and speed, pre-1.0, with no controls of its own: gpui-component supplies them.
5. **GTK 4 or Qt.** Native on Linux or everywhere, at the cost of a large runtime and, for Qt, a C++ toolchain.

## Decision

Option 4.
The user likes how Zed feels, and accepted the costs we listed: an API that changes, weak screen-reader support, and an unknown size.

The spike built every control the app needs on Windows with `gpui-kit` 0.9.1, which pins `gpui-pre` 0.3.12, a weekly snapshot of Zed's gpui: a code input, a 60-row table whose checkboxes toggle, a tooltip, and a status line a worker thread updates every second ([interface log](../areas/interface/log.md#2026-11-13-gpui-kit-builds-every-control-the-window-needs-on-windows-and-the-exe-is-about-16-mb)).

## What it commits us to

- `gpui-kit` and `gpui-pre` are pinned to exact versions and updated on purpose, since a minor version of `gpui-kit` comes out about every two weeks.
- Zed's own `ui`, `theme`, and `settings` crates are GPL-3.0-or-later, so nothing is copied from them; gpui, gpui-component, and `gpui-kit` are Apache-2.0 (read in their `Cargo.toml` files, 2026-11-13).
- A release build takes about 4 minutes from clean on the development machine, so day-to-day work uses the dev profile.
- A Linux build links fontconfig, freetype, xkbcommon, X11, and xcb, and loads Wayland, EGL, and Vulkan at run time (read in the spike's dependencies; the Linux build is unverified), so CI installs their development packages and the AppImage needs a check on a Steam Deck.
