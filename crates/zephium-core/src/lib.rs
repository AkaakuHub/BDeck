#![forbid(unsafe_code)]

pub mod accelerator;
/// Whether a tab is making sound, and whether the person muted it.
pub mod audio;
pub mod certificates;
pub mod commands;
pub mod deck;
pub mod downloads;
pub mod extensions;
pub mod geometry;
pub mod http_auth;
pub mod icon;
pub mod ids;
pub mod item;
pub mod items;
pub mod layout;
pub mod macos;
/// What a tab's media is doing, and which tab the sidebar's player follows.
pub mod media;
pub mod navigation;
pub mod permissions;
pub mod ports;
pub mod profiles;
pub mod proxy;
pub mod runtime_security;
pub mod session;
pub mod spaces;
pub mod split;
pub mod webkitgtk;
pub mod webview2;
pub mod windows;
/// Page zoom levels and their bounds.
pub mod zoom;

/// Allowlisted application appearance preferences.
pub mod preferences;
/// Colour themes and the window ground native derives from them.
pub mod theme;
