//! Layout & Controls styles (control-layouts, capability `ui-controls`): the closed list of keyboard
//! commands, what each style binds them to, where each style puts the panels, what the wheel does,
//! and the settings code that carries all of it.
//!
//! The owner (`docs/ktprompts/v26.10.txt`, L1, L2): "предлагать выбрать "Layout & Controls style" -
//! как в самых известных коммерческих программах. Первой плиточкой идет наша и подпись "like
//! Vegas"" and "кастомизировать всё, и в итоге получить "код сохранения" настроек, чтобы можно было
//! при переустановке его ввести (как в старых играх сохранения)".
//!
//! This file is pure: no window, no store. `handle_keys` asks [`Controls::pressed`], the menus and
//! the hints ask [`Controls::label`], the panels ask `choice.layout`, the timeline asks
//! `choice.wheel`: one table, so a key and its label cannot disagree (rule 17).

use crate::i18n::{Key as Text, k};
use eframe::egui;
use egui::Key;
use std::collections::HashMap;

/// A keyboard command. The number is what the settings code stores: **frozen, append only**, a
/// code given today must open tomorrow (t1702).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Cmd {
    PlayPause = 0,
    NavPrev = 1,
    NavNext = 2,
    ProjectStart = 3,
    ProjectEnd = 4,
    FramePrev = 5,
    FrameNext = 6,
    GotoTimecode = 7,
    ShuttleBack = 8,
    ShuttleStop = 9,
    ShuttleForward = 10,
    Split = 11,
    Delete = 12,
    Copy = 13,
    Paste = 14,
    Undo = 15,
    Redo = 16,
    Save = 17,
    SaveAs = 18,
    NewProject = 19,
    OpenProject = 20,
    Render = 21,
    Find = 22,
    TrackMute = 23,
    TrackSolo = 24,
    RegionIn = 25,
    RegionOut = 26,
    RegionPrev = 27,
    RegionNext = 28,
    RegionClear = 29,
    Loop = 30,
    Marker = 31,
    ZoomIn = 32,
    ZoomOut = 33,
    ZoomFit = 34,
    SelectTool = 35,
}

/// The groups of the key list, in the order a person reads them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Transport,
    Edit,
    Region,
    View,
    File,
}

impl Group {
    pub const ALL: [Group; 5] = [Group::Transport, Group::Edit, Group::Region, Group::View, Group::File];

    pub fn title(self) -> Text {
        match self {
            Group::Transport => k::controls::group::TRANSPORT,
            Group::Edit => k::controls::group::EDIT,
            Group::Region => k::controls::group::REGION,
            Group::View => k::controls::group::VIEW,
            Group::File => k::controls::group::FILE,
        }
    }
}

/// `(command, group, name)`: every command, in list order. Full catalog paths: t125 looks a key up
/// by its path.
pub const COMMANDS: &[(Cmd, Group, Text)] = &[
    (Cmd::PlayPause, Group::Transport, k::controls::cmd::PLAY_PAUSE),
    (Cmd::NavPrev, Group::Transport, k::controls::cmd::NAV_PREV),
    (Cmd::NavNext, Group::Transport, k::controls::cmd::NAV_NEXT),
    (Cmd::ProjectStart, Group::Transport, k::controls::cmd::PROJECT_START),
    (Cmd::ProjectEnd, Group::Transport, k::controls::cmd::PROJECT_END),
    (Cmd::FramePrev, Group::Transport, k::controls::cmd::FRAME_PREV),
    (Cmd::FrameNext, Group::Transport, k::controls::cmd::FRAME_NEXT),
    (Cmd::ShuttleBack, Group::Transport, k::controls::cmd::SHUTTLE_BACK),
    (Cmd::ShuttleStop, Group::Transport, k::controls::cmd::SHUTTLE_STOP),
    (Cmd::ShuttleForward, Group::Transport, k::controls::cmd::SHUTTLE_FORWARD),
    (Cmd::GotoTimecode, Group::Transport, k::controls::cmd::GOTO_TIMECODE),
    (Cmd::Split, Group::Edit, k::controls::cmd::SPLIT),
    (Cmd::Delete, Group::Edit, k::controls::cmd::DELETE),
    (Cmd::Copy, Group::Edit, k::controls::cmd::COPY),
    (Cmd::Paste, Group::Edit, k::controls::cmd::PASTE),
    (Cmd::Undo, Group::Edit, k::controls::cmd::UNDO),
    (Cmd::Redo, Group::Edit, k::controls::cmd::REDO),
    (Cmd::TrackMute, Group::Edit, k::controls::cmd::TRACK_MUTE),
    (Cmd::TrackSolo, Group::Edit, k::controls::cmd::TRACK_SOLO),
    (Cmd::SelectTool, Group::Edit, k::controls::cmd::SELECT_TOOL),
    (Cmd::Find, Group::Edit, k::controls::cmd::FIND),
    (Cmd::RegionIn, Group::Region, k::controls::cmd::REGION_IN),
    (Cmd::RegionOut, Group::Region, k::controls::cmd::REGION_OUT),
    (Cmd::RegionPrev, Group::Region, k::controls::cmd::REGION_PREV),
    (Cmd::RegionNext, Group::Region, k::controls::cmd::REGION_NEXT),
    (Cmd::RegionClear, Group::Region, k::controls::cmd::REGION_CLEAR),
    (Cmd::Loop, Group::Region, k::controls::cmd::LOOP),
    (Cmd::Marker, Group::Region, k::controls::cmd::MARKER),
    (Cmd::ZoomIn, Group::View, k::controls::cmd::ZOOM_IN),
    (Cmd::ZoomOut, Group::View, k::controls::cmd::ZOOM_OUT),
    (Cmd::ZoomFit, Group::View, k::controls::cmd::ZOOM_FIT),
    (Cmd::Save, Group::File, k::controls::cmd::SAVE),
    (Cmd::SaveAs, Group::File, k::controls::cmd::SAVE_AS),
    (Cmd::NewProject, Group::File, k::controls::cmd::NEW_PROJECT),
    (Cmd::OpenProject, Group::File, k::controls::cmd::OPEN_PROJECT),
    (Cmd::Render, Group::File, k::controls::cmd::RENDER),
];

/// The number of commands: one past the highest frozen number.
const CMD_COUNT: usize = 36;

fn cmd_from(code: u8) -> Option<Cmd> {
    COMMANDS.iter().map(|c| c.0).find(|c| *c as u8 == code)
}

/// The name of a command in the current language.
pub fn cmd_name(cmd: Cmd) -> &'static str {
    COMMANDS.iter().find(|c| c.0 == cmd).map_or("", |c| crate::i18n::tr(c.2))
}

/// A key with its modifiers.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Chord {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

const fn p(key: Key) -> Chord {
    Chord { key, ctrl: false, shift: false, alt: false }
}
const fn c(key: Key) -> Chord {
    Chord { key, ctrl: true, shift: false, alt: false }
}
const fn s(key: Key) -> Chord {
    Chord { key, ctrl: false, shift: true, alt: false }
}
const fn a(key: Key) -> Chord {
    Chord { key, ctrl: false, shift: false, alt: true }
}
const fn cs(key: Key) -> Chord {
    Chord { key, ctrl: true, shift: true, alt: false }
}
const fn ca(key: Key) -> Chord {
    Chord { key, ctrl: true, shift: false, alt: true }
}
const fn sa(key: Key) -> Chord {
    Chord { key, ctrl: false, shift: true, alt: true }
}

impl Chord {
    pub fn of(key: Key, m: egui::Modifiers) -> Chord {
        Chord { key, ctrl: m.ctrl, shift: m.shift, alt: m.alt }
    }

    /// What is printed: the modifiers in one fixed order, then the key. `Del` as the menus always
    /// printed it; everything else by egui's name.
    pub fn label(self) -> String {
        let key = match self.key {
            Key::Delete => "Del",
            other => other.symbol_or_name(),
        };
        let mut out = String::new();
        for (on, name) in [(self.ctrl, "Ctrl+"), (self.shift, "Shift+"), (self.alt, "Alt+")] {
            if on {
                out.push_str(name);
            }
        }
        out.push_str(key);
        out
    }

    fn bare(self) -> bool {
        !self.ctrl && !self.shift && !self.alt
    }
}

/// Esc and Enter answer in every window and every field, so they are never a command's key.
pub fn reserved(key: Key) -> bool {
    matches!(key, Key::Escape | Key::Enter)
}

/// Where the panels stand. The number is what the code stores: frozen, append only.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Arrangement {
    /// The program's own: the bin on the left, the timeline across the bottom, the meters beside
    /// the preview.
    Classic = 0,
    /// The timeline on top, the bin, the preview and the meters under it.
    TimelineOnTop = 1,
    /// The preview over the whole width; the bin, the timeline and the meters in a row under it.
    BinBesideTimeline = 2,
    /// The bin and the preview on top, the timeline with the meters at its right end under them.
    MetersBesideTimeline = 3,
}

impl Arrangement {
    pub const ALL: [Arrangement; 4] = [
        Arrangement::Classic,
        Arrangement::TimelineOnTop,
        Arrangement::BinBesideTimeline,
        Arrangement::MetersBesideTimeline,
    ];

    fn from_code(code: u8) -> Option<Arrangement> {
        Self::ALL.into_iter().find(|a| *a as u8 == code)
    }

    pub fn name(self) -> Text {
        match self {
            Arrangement::Classic => k::controls::layout::CLASSIC,
            Arrangement::TimelineOnTop => k::controls::layout::TIMELINE_ON_TOP,
            Arrangement::BinBesideTimeline => k::controls::layout::BIN_BESIDE_TIMELINE,
            Arrangement::MetersBesideTimeline => k::controls::layout::METERS_BESIDE_TIMELINE,
        }
    }
}

/// What a wheel turn over the tracks does. The number is stored: frozen, append only.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum WheelAct {
    Time = 0,
    Tracks = 1,
    Zoom = 2,
}

impl WheelAct {
    pub const ALL: [WheelAct; 3] = [WheelAct::Time, WheelAct::Tracks, WheelAct::Zoom];

    fn from_code(code: u8) -> Option<WheelAct> {
        Self::ALL.into_iter().find(|a| *a as u8 == code)
    }

    pub fn name(self) -> Text {
        match self {
            WheelAct::Time => k::controls::wheel::TIME,
            WheelAct::Tracks => k::controls::wheel::TRACKS,
            WheelAct::Zoom => k::controls::wheel::ZOOM,
        }
    }
}

/// The wheel with each modifier. Ctrl+Shift+wheel stays the track height in every style, a pinch
/// stays the zoom: neither has a word in the styles the person comes from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Wheel {
    pub plain: WheelAct,
    pub ctrl: WheelAct,
    pub shift: WheelAct,
    pub alt: WheelAct,
}

impl Wheel {
    /// The owner's: wheel along time, Ctrl zooms, Shift walks the tracks (the port of v1).
    pub const OWN: Wheel = Wheel { plain: WheelAct::Time, ctrl: WheelAct::Zoom, shift: WheelAct::Tracks, alt: WheelAct::Time };

    pub fn act(self, ctrl: bool, shift: bool, alt: bool) -> WheelAct {
        if ctrl {
            self.ctrl
        } else if shift {
            self.shift
        } else if alt {
            self.alt
        } else {
            self.plain
        }
    }

    /// The modifier that zooms, as a prefix ("Ctrl+", or nothing for the bare wheel); `None` when
    /// no wheel row zooms.
    pub fn zoom_prefix(self) -> Option<&'static str> {
        [(self.plain, ""), (self.ctrl, "Ctrl+"), (self.alt, "Alt+"), (self.shift, "Shift+")]
            .into_iter()
            .find(|(a, _)| *a == WheelAct::Zoom)
            .map(|(_, p)| p)
    }

    /// The clause of the timeline's hint: the bare wheel, then each modifier that differs from it.
    pub fn hint(self) -> String {
        use crate::i18n::tr;
        let word = |a: WheelAct| {
            tr(match a {
                WheelAct::Time => k::controls::wheel::SHORT_TIME,
                WheelAct::Tracks => k::controls::wheel::SHORT_TRACKS,
                WheelAct::Zoom => k::controls::wheel::SHORT_ZOOM,
            })
        };
        let mut out = format!("{} — {}", tr(k::controls::wheel::WORD), word(self.plain));
        for (act, name) in [(self.ctrl, "Ctrl"), (self.shift, "Shift"), (self.alt, "Alt")] {
            if act != self.plain {
                out.push_str(&format!(", {name} — {}", word(act)));
            }
        }
        out
    }

    fn pack(self) -> u8 {
        self.plain as u8 | (self.ctrl as u8) << 2 | (self.shift as u8) << 4 | (self.alt as u8) << 6
    }

    fn unpack(b: u8) -> Option<Wheel> {
        Some(Wheel {
            plain: WheelAct::from_code(b & 3)?,
            ctrl: WheelAct::from_code(b >> 2 & 3)?,
            shift: WheelAct::from_code(b >> 4 & 3)?,
            alt: WheelAct::from_code(b >> 6 & 3)?,
        })
    }
}

/// A tile. The number is stored: frozen, append only.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Style {
    /// Ours, captioned "like Vegas": the owner's own default, untouched.
    Own = 0,
    Premiere = 1,
    Resolve = 2,
    FinalCut = 3,
    AfterEffects = 4,
}

impl Style {
    /// The tile order: ours first (L1).
    pub const ALL: [Style; 5] = [Style::Own, Style::Premiere, Style::Resolve, Style::FinalCut, Style::AfterEffects];

    fn from_code(code: u8) -> Option<Style> {
        Self::ALL.into_iter().find(|a| *a as u8 == code)
    }

    /// The name on the tile. Product names are not translated; the tile says "like …" around it.
    pub fn product(self) -> &'static str {
        match self {
            Style::Own => "Vegas",
            Style::Premiere => "Premiere Pro",
            Style::Resolve => "DaVinci Resolve",
            Style::FinalCut => "Final Cut Pro",
            Style::AfterEffects => "After Effects",
        }
    }

    /// What that program's hands reach for and we cannot give: said on the tile, not discovered.
    pub fn missing(self) -> Text {
        match self {
            Style::Own => k::controls::missing::OWN,
            Style::Premiere => k::controls::missing::PREMIERE,
            Style::Resolve => k::controls::missing::RESOLVE,
            Style::FinalCut => k::controls::missing::FINAL_CUT,
            Style::AfterEffects => k::controls::missing::AFTER_EFFECTS,
        }
    }

    pub fn layout(self) -> Arrangement {
        match self {
            Style::Own | Style::FinalCut | Style::AfterEffects => Arrangement::Classic,
            Style::Premiere => Arrangement::BinBesideTimeline,
            Style::Resolve => Arrangement::MetersBesideTimeline,
        }
    }

    pub fn wheel(self) -> Wheel {
        use WheelAct::*;
        match self {
            Style::Own => Wheel::OWN,
            // Alt zooms in all four (design D5); the rest is from the note's table
            Style::Premiere => Wheel { plain: Time, ctrl: Tracks, shift: Tracks, alt: Zoom },
            Style::Resolve | Style::FinalCut | Style::AfterEffects => Wheel { plain: Tracks, ctrl: Time, shift: Time, alt: Zoom },
        }
    }

    pub fn bindings(self) -> &'static [(Cmd, &'static [Chord])] {
        match self {
            Style::Own => OWN,
            Style::Premiere => PREMIERE,
            Style::Resolve => RESOLVE,
            Style::FinalCut => FINAL_CUT,
            Style::AfterEffects => AFTER_EFFECTS,
        }
    }

    fn keys_of(self, cmd: Cmd) -> &'static [Chord] {
        self.bindings().iter().find(|b| b.0 == cmd).map_or(&[], |b| b.1)
    }
}

/// The owner's keys, each set at his direct request over the rounds: not a choice of this file.
/// Space is the transport machine of `handle_keys` (rounds 6, 16, R.72), in every style.
const OWN: &[(Cmd, &[Chord])] = &[
    (Cmd::PlayPause, &[p(Key::Space)]),
    (Cmd::NavPrev, &[p(Key::A), p(Key::Home)]),
    (Cmd::NavNext, &[p(Key::D), p(Key::End)]),
    (Cmd::ProjectStart, &[s(Key::Home)]),
    (Cmd::ProjectEnd, &[s(Key::End)]),
    (Cmd::FramePrev, &[p(Key::ArrowLeft)]),
    (Cmd::FrameNext, &[p(Key::ArrowRight)]),
    (Cmd::GotoTimecode, &[c(Key::L)]),
    (Cmd::Split, &[p(Key::S)]),
    (Cmd::Delete, &[p(Key::Delete), p(Key::Backspace)]),
    (Cmd::Copy, &[c(Key::C)]),
    (Cmd::Paste, &[c(Key::V)]),
    (Cmd::Undo, &[c(Key::Z)]),
    (Cmd::Redo, &[cs(Key::Z), c(Key::Y)]),
    (Cmd::Save, &[c(Key::S)]),
    (Cmd::Find, &[c(Key::F)]),
    (Cmd::TrackMute, &[a(Key::M)]),
    (Cmd::TrackSolo, &[a(Key::S)]),
    (Cmd::RegionIn, &[p(Key::I)]),
    (Cmd::RegionOut, &[p(Key::O)]),
    (Cmd::RegionPrev, &[s(Key::A)]),
    (Cmd::RegionNext, &[s(Key::D)]),
    (Cmd::Loop, &[p(Key::L)]),
    (Cmd::Marker, &[p(Key::M)]),
];

const PREMIERE: &[(Cmd, &[Chord])] = &[
    (Cmd::PlayPause, &[p(Key::Space)]),
    (Cmd::NavPrev, &[p(Key::ArrowUp)]),
    (Cmd::NavNext, &[p(Key::ArrowDown)]),
    (Cmd::ProjectStart, &[p(Key::Home)]),
    (Cmd::ProjectEnd, &[p(Key::End)]),
    (Cmd::FramePrev, &[p(Key::ArrowLeft)]),
    (Cmd::FrameNext, &[p(Key::ArrowRight)]),
    (Cmd::ShuttleBack, &[p(Key::J)]),
    (Cmd::ShuttleStop, &[p(Key::K)]),
    (Cmd::ShuttleForward, &[p(Key::L)]),
    (Cmd::Split, &[c(Key::K)]),
    (Cmd::Delete, &[p(Key::Delete), p(Key::Backspace)]),
    (Cmd::Copy, &[c(Key::C)]),
    (Cmd::Paste, &[c(Key::V)]),
    (Cmd::Undo, &[c(Key::Z)]),
    (Cmd::Redo, &[cs(Key::Z)]),
    (Cmd::Save, &[c(Key::S)]),
    (Cmd::SaveAs, &[cs(Key::S)]),
    (Cmd::NewProject, &[ca(Key::N)]),
    (Cmd::OpenProject, &[c(Key::O)]),
    (Cmd::Render, &[c(Key::M)]),
    (Cmd::Find, &[c(Key::F)]),
    (Cmd::RegionIn, &[p(Key::I)]),
    (Cmd::RegionOut, &[p(Key::O)]),
    (Cmd::RegionClear, &[cs(Key::X)]),
    (Cmd::Loop, &[c(Key::L)]),
    (Cmd::Marker, &[p(Key::M)]),
    (Cmd::ZoomIn, &[p(Key::Equals)]),
    (Cmd::ZoomOut, &[p(Key::Minus)]),
    (Cmd::ZoomFit, &[p(Key::Backslash)]),
];

const RESOLVE: &[(Cmd, &[Chord])] = &[
    (Cmd::PlayPause, &[p(Key::Space)]),
    (Cmd::NavPrev, &[p(Key::ArrowUp)]),
    (Cmd::NavNext, &[p(Key::ArrowDown)]),
    (Cmd::ProjectStart, &[p(Key::Home)]),
    (Cmd::ProjectEnd, &[p(Key::End)]),
    (Cmd::FramePrev, &[p(Key::ArrowLeft)]),
    (Cmd::FrameNext, &[p(Key::ArrowRight)]),
    (Cmd::ShuttleBack, &[p(Key::J)]),
    (Cmd::ShuttleStop, &[p(Key::K)]),
    (Cmd::ShuttleForward, &[p(Key::L)]),
    (Cmd::Split, &[c(Key::Backslash), c(Key::B)]),
    (Cmd::Delete, &[p(Key::Delete), p(Key::Backspace)]),
    (Cmd::Copy, &[c(Key::C)]),
    (Cmd::Paste, &[c(Key::V)]),
    (Cmd::Undo, &[c(Key::Z)]),
    (Cmd::Redo, &[cs(Key::Z)]),
    (Cmd::Save, &[c(Key::S)]),
    (Cmd::SaveAs, &[cs(Key::S)]),
    (Cmd::RegionIn, &[p(Key::I)]),
    (Cmd::RegionOut, &[p(Key::O)]),
    (Cmd::RegionClear, &[a(Key::X)]),
    (Cmd::Loop, &[c(Key::Slash)]),
    (Cmd::Marker, &[p(Key::M)]),
    (Cmd::ZoomIn, &[c(Key::Equals)]),
    (Cmd::ZoomOut, &[c(Key::Minus)]),
    (Cmd::ZoomFit, &[s(Key::Z)]),
];

const FINAL_CUT: &[(Cmd, &[Chord])] = &[
    (Cmd::PlayPause, &[p(Key::Space)]),
    (Cmd::NavPrev, &[p(Key::ArrowUp), p(Key::Semicolon)]),
    (Cmd::NavNext, &[p(Key::ArrowDown), p(Key::Quote)]),
    (Cmd::ProjectStart, &[p(Key::Home)]),
    (Cmd::ProjectEnd, &[p(Key::End)]),
    (Cmd::FramePrev, &[p(Key::ArrowLeft)]),
    (Cmd::FrameNext, &[p(Key::ArrowRight)]),
    (Cmd::ShuttleBack, &[p(Key::J)]),
    (Cmd::ShuttleStop, &[p(Key::K)]),
    (Cmd::ShuttleForward, &[p(Key::L)]),
    (Cmd::Split, &[c(Key::B)]),
    (Cmd::Delete, &[p(Key::Delete), p(Key::Backspace)]),
    (Cmd::Copy, &[c(Key::C)]),
    (Cmd::Paste, &[c(Key::V)]),
    (Cmd::Undo, &[c(Key::Z)]),
    (Cmd::Redo, &[cs(Key::Z)]),
    (Cmd::Save, &[c(Key::S)]),
    (Cmd::SaveAs, &[cs(Key::S)]),
    (Cmd::NewProject, &[c(Key::N)]),
    (Cmd::OpenProject, &[c(Key::O)]),
    (Cmd::Render, &[c(Key::E)]),
    (Cmd::Find, &[c(Key::F)]),
    (Cmd::RegionIn, &[p(Key::I)]),
    (Cmd::RegionOut, &[p(Key::O)]),
    (Cmd::RegionClear, &[a(Key::X)]),
    (Cmd::Loop, &[c(Key::L)]),
    (Cmd::Marker, &[p(Key::M)]),
    (Cmd::ZoomIn, &[c(Key::Equals)]),
    (Cmd::ZoomOut, &[c(Key::Minus)]),
    (Cmd::ZoomFit, &[s(Key::Z)]),
    (Cmd::GotoTimecode, &[c(Key::P)]),
];

const AFTER_EFFECTS: &[(Cmd, &[Chord])] = &[
    (Cmd::PlayPause, &[p(Key::Space)]),
    (Cmd::NavPrev, &[p(Key::J)]),
    (Cmd::NavNext, &[p(Key::K)]),
    (Cmd::ProjectStart, &[p(Key::Home)]),
    (Cmd::ProjectEnd, &[p(Key::End)]),
    (Cmd::FramePrev, &[p(Key::PageUp)]),
    (Cmd::FrameNext, &[p(Key::PageDown)]),
    (Cmd::GotoTimecode, &[sa(Key::J)]),
    (Cmd::Split, &[cs(Key::D)]),
    (Cmd::Delete, &[p(Key::Delete), p(Key::Backspace)]),
    (Cmd::Copy, &[c(Key::C)]),
    (Cmd::Paste, &[c(Key::V)]),
    (Cmd::Undo, &[c(Key::Z)]),
    (Cmd::Redo, &[cs(Key::Z)]),
    (Cmd::Save, &[c(Key::S)]),
    (Cmd::SaveAs, &[cs(Key::S)]),
    (Cmd::NewProject, &[ca(Key::N)]),
    (Cmd::OpenProject, &[c(Key::O)]),
    (Cmd::Render, &[c(Key::M)]),
    (Cmd::RegionIn, &[p(Key::B)]),
    (Cmd::RegionOut, &[p(Key::N)]),
    // the numpad's * has no key of its own in egui: Shift+8 is the same star
    (Cmd::Marker, &[s(Key::Num8)]),
    (Cmd::Find, &[c(Key::F)]),
    (Cmd::ZoomIn, &[p(Key::Equals)]),
    (Cmd::ZoomOut, &[p(Key::Minus)]),
    (Cmd::ZoomFit, &[s(Key::Semicolon)]),
];

/// The keys a code can carry, by number: **frozen, append only** (the position is the number).
/// Our own table, because `egui::Key`'s numbering changes with egui.
const KEYS: &[Key] = &[
    Key::A, Key::B, Key::C, Key::D, Key::E, Key::F, Key::G, Key::H, Key::I, Key::J, Key::K, Key::L, Key::M,
    Key::N, Key::O, Key::P, Key::Q, Key::R, Key::S, Key::T, Key::U, Key::V, Key::W, Key::X, Key::Y, Key::Z,
    Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9,
    Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6, Key::F7, Key::F8, Key::F9, Key::F10, Key::F11, Key::F12,
    Key::ArrowLeft, Key::ArrowRight, Key::ArrowUp, Key::ArrowDown,
    Key::Space, Key::Tab, Key::Backspace, Key::Delete, Key::Insert, Key::Home, Key::End, Key::PageUp, Key::PageDown,
    Key::Minus, Key::Equals, Key::Comma, Key::Period, Key::Semicolon, Key::Slash, Key::Backslash,
    Key::OpenBracket, Key::CloseBracket, Key::Quote, Key::Backtick,
];

/// Whether a key can be given to a command at all (what the code can carry, and not Esc, Enter).
pub fn assignable(key: Key) -> bool {
    KEYS.contains(&key)
}

/// A style plus what the person changed on top of it. This is exactly what the code carries, and
/// what `start.json` keeps: what is not here does not survive a reinstall.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub style: Style,
    pub layout: Arrangement,
    pub wheel: Wheel,
    /// The commands whose keys differ from the style's, sorted by number; an empty list means
    /// "this command has no key". What equals the style is never stored.
    pub keys: Vec<(Cmd, Vec<Chord>)>,
}

impl Default for Choice {
    fn default() -> Choice {
        Choice::of(Style::Own)
    }
}

/// Why a code was refused: each with its own words, nothing is applied (rule 16).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CodeError {
    /// Letters that are not in the code's alphabet, or too short to be a code.
    Unreadable,
    /// The check digits disagree: a typo.
    Typo,
    /// Made by a newer Kinetren.
    Newer,
}

impl CodeError {
    pub fn text(self) -> Text {
        match self {
            CodeError::Unreadable => k::controls::code::UNREADABLE,
            CodeError::Typo => k::controls::code::TYPO,
            CodeError::Newer => k::controls::code::NEWER,
        }
    }
}

const CODE_VERSION: u8 = 1;
/// Crockford's base 32: no I, L, O, U, so what a person misreads cannot be a different letter.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const CODE_GROUP: usize = 5;
/// Keys per command a code can carry (two bits of the header byte): the list offers no more.
pub const MAX_CHORDS: usize = 3;

fn crc16(bytes: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for b in bytes {
        crc ^= (*b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { crc << 1 ^ 0x1021 } else { crc << 1 };
        }
    }
    crc
}

fn to_base32(bytes: &[u8]) -> String {
    let mut out = String::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for b in bytes {
        acc = acc << 8 | *b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[(acc >> bits & 31) as usize] as char);
        }
        acc &= (1 << bits) - 1;
    }
    if bits > 0 {
        out.push(ALPHABET[(acc << (5 - bits) & 31) as usize] as char);
    }
    out
}

fn from_base32(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for ch in text.chars() {
        if ch == '-' || ch.is_whitespace() {
            continue;
        }
        // what a person reads from paper: lower case, O for zero, I and L for one
        let ch = match ch.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        };
        let v = ALPHABET.iter().position(|a| *a as char == ch)? as u32;
        acc = acc << 5 | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 255) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

impl Choice {
    pub fn of(style: Style) -> Choice {
        Choice { style, layout: style.layout(), wheel: style.wheel(), keys: Vec::new() }
    }

    /// Nothing changed on top of the tile.
    pub fn is_factory(&self) -> bool {
        *self == Choice::of(self.style)
    }

    /// The keys of a command as they stand.
    pub fn keys_of(&self, cmd: Cmd) -> &[Chord] {
        match self.keys.iter().find(|o| o.0 == cmd) {
            Some(o) => &o.1,
            None => self.style.keys_of(cmd),
        }
    }

    pub fn is_changed(&self, cmd: Cmd) -> bool {
        self.keys.iter().any(|o| o.0 == cmd)
    }

    /// Replace the keys of a command; what equals the style is dropped, so a code never carries
    /// a change that changes nothing.
    pub fn set_keys(&mut self, cmd: Cmd, keys: Vec<Chord>) {
        self.keys.retain(|o| o.0 != cmd);
        if keys != self.style.keys_of(cmd) {
            self.keys.push((cmd, keys));
            self.keys.sort_by_key(|o| o.0);
        }
    }

    /// The key at `slot` becomes `chord` (`None` — one more key).
    pub fn bind(&mut self, cmd: Cmd, slot: Option<usize>, chord: Chord) {
        let mut keys = self.keys_of(cmd).to_vec();
        match slot.filter(|s| *s < keys.len()) {
            Some(s) => keys[s] = chord,
            None => keys.push(chord),
        }
        self.set_keys(cmd, keys);
    }

    /// The key at `slot` goes away.
    pub fn unbind(&mut self, cmd: Cmd, slot: usize) {
        let mut keys = self.keys_of(cmd).to_vec();
        if slot < keys.len() {
            keys.remove(slot);
            self.set_keys(cmd, keys);
        }
    }

    /// A command gives one key up: the person agreed to take it from there.
    pub fn release(&mut self, cmd: Cmd, chord: Chord) {
        let keys = self.keys_of(cmd).iter().copied().filter(|c| *c != chord).collect();
        self.set_keys(cmd, keys);
    }

    /// The command back to the style's keys.
    pub fn reset_keys(&mut self, cmd: Cmd) {
        self.keys.retain(|o| o.0 != cmd);
    }

    /// The code: version, style, layout, wheel, the changed commands, a CRC-16 of all of it, in
    /// groups of five letters. Everything is a number from a frozen table.
    pub fn to_code(&self) -> String {
        let mut b = vec![CODE_VERSION, self.style as u8, self.layout as u8, self.wheel.pack()];
        for (cmd, keys) in &self.keys {
            b.push(*cmd as u8 | (keys.len().min(MAX_CHORDS) as u8) << 6);
            for chord in keys.iter().take(MAX_CHORDS) {
                let n = KEYS.iter().position(|k| *k == chord.key).unwrap_or(0) as u8;
                b.push(n);
                b.push(chord.ctrl as u8 | (chord.shift as u8) << 1 | (chord.alt as u8) << 2);
            }
        }
        let crc = crc16(&b);
        b.extend(crc.to_be_bytes());
        let text = to_base32(&b);
        let groups: Vec<&str> = text.as_bytes().chunks(CODE_GROUP).map(|g| std::str::from_utf8(g).unwrap_or("")).collect();
        groups.join("-")
    }

    pub fn from_code(text: &str) -> Result<Choice, CodeError> {
        let mut b = from_base32(text).ok_or(CodeError::Unreadable)?;
        // a to_base32 tail of under one byte decodes to nothing: there is nothing to strip
        if b.len() < 6 {
            return Err(CodeError::Unreadable);
        }
        let crc = u16::from_be_bytes([b[b.len() - 2], b[b.len() - 1]]);
        b.truncate(b.len() - 2);
        if crc16(&b) != crc {
            return Err(CodeError::Typo);
        }
        if b[0] > CODE_VERSION {
            return Err(CodeError::Newer);
        }
        let style = Style::from_code(b[1]).ok_or(CodeError::Newer)?;
        let layout = Arrangement::from_code(b[2]).ok_or(CodeError::Newer)?;
        let wheel = Wheel::unpack(b[3]).ok_or(CodeError::Newer)?;
        let mut keys: Vec<(Cmd, Vec<Chord>)> = Vec::new();
        let mut at = 4;
        while at < b.len() {
            let head = b[at];
            at += 1;
            let cmd = cmd_from(head & 63).ok_or(CodeError::Newer)?;
            let mut list = Vec::new();
            for _ in 0..head >> 6 {
                let (Some(n), Some(m)) = (b.get(at), b.get(at + 1)) else {
                    return Err(CodeError::Unreadable);
                };
                at += 2;
                let key = *KEYS.get(*n as usize).ok_or(CodeError::Newer)?;
                list.push(Chord { key, ctrl: m & 1 != 0, shift: m & 2 != 0, alt: m & 4 != 0 });
            }
            keys.push((cmd, list));
        }
        Ok(Choice { style, layout, wheel, keys })
    }
}

/// The commands pressed this frame.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct CmdSet(u64);

impl CmdSet {
    pub fn add(&mut self, cmd: Cmd) {
        self.0 |= 1 << cmd as u8;
    }

    pub fn has(self, cmd: Cmd) -> bool {
        self.0 & 1 << cmd as u8 != 0
    }

    pub fn remove(&mut self, cmd: Cmd) {
        self.0 &= !(1 << cmd as u8);
    }
}

/// A [`Choice`] compiled once for the frames: the lookup and the printed labels, so a draw path
/// neither searches nor allocates (invariant 12).
pub struct Controls {
    pub choice: Choice,
    map: HashMap<Chord, Cmd>,
    labels: [String; CMD_COUNT],
    labels_all: [String; CMD_COUNT],
    play_key: Option<Key>,
}

impl Default for Controls {
    fn default() -> Controls {
        Controls::new(Choice::default())
    }
}

/// A command with no key prints this in a hint.
const NO_KEY: &str = "—";

impl Controls {
    pub fn new(choice: Choice) -> Controls {
        let mut map = HashMap::new();
        let mut labels: [String; CMD_COUNT] = std::array::from_fn(|_| String::new());
        let mut labels_all: [String; CMD_COUNT] = std::array::from_fn(|_| NO_KEY.to_string());
        for (cmd, _, _) in COMMANDS {
            let keys = choice.keys_of(*cmd);
            for chord in keys {
                map.entry(*chord).or_insert(*cmd);
            }
            if let Some(first) = keys.first() {
                labels[*cmd as usize] = first.label();
                labels_all[*cmd as usize] = keys.iter().map(|c| c.label()).collect::<Vec<_>>().join(" / ");
            }
        }
        let play_key = choice.keys_of(Cmd::PlayPause).first().map(|c| c.key);
        Controls { choice, map, labels, labels_all, play_key }
    }

    /// The first key of a command, for a menu item's right side; empty — it has none.
    pub fn label(&self, cmd: Cmd) -> &str {
        &self.labels[cmd as usize]
    }

    /// All its keys, for a hint: `—` when it has none.
    pub fn label_all(&self, cmd: Cmd) -> &str {
        &self.labels_all[cmd as usize]
    }

    /// The key of the transport machine: tap, double tap, hold, Ctrl+hold.
    pub fn play_key(&self) -> Option<Key> {
        self.play_key
    }

    /// Who owns exactly this chord. The transport machine owns Ctrl+its key too (reverse hold).
    pub fn holder(&self, chord: Chord) -> Option<Cmd> {
        if self.play_key == Some(chord.key) && !chord.shift && !chord.alt {
            return Some(Cmd::PlayPause);
        }
        self.map.get(&chord).copied()
    }

    /// What a press means. An exact chord first; the old handler was lax about modifiers nobody
    /// bound (Shift+S split, Ctrl+Home jumped), and a default that does not change keeps that: an
    /// unbound Shift falls off every key but the arrows, and any unbound modifier falls off the
    /// keys that have no letter to protect.
    pub fn lookup(&self, chord: Chord) -> Option<Cmd> {
        if let Some(cmd) = self.map.get(&chord) {
            return Some(*cmd);
        }
        let arrow = matches!(chord.key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown);
        if chord.shift && !arrow {
            let less = Chord { shift: false, ..chord };
            if let Some(cmd) = self.map.get(&less) {
                return Some(*cmd);
            }
        }
        if matches!(chord.key, Key::Delete | Key::Backspace | Key::Home | Key::End) && !chord.bare() {
            return self.map.get(&p(chord.key)).copied();
        }
        None
    }

    /// The commands of this frame, from the same events the old `key_pressed` read: an autorepeat
    /// IS a press here (a held arrow keeps stepping). egui-winit turns Ctrl+C, Ctrl+V and Ctrl+X
    /// into their own events (item 21), so those come back to the chord a person pressed.
    pub fn pressed(&self, i: &egui::InputState) -> CmdSet {
        let mut set = CmdSet::default();
        for e in &i.events {
            let chord = match e {
                egui::Event::Key { key, pressed: true, .. } => Chord::of(*key, i.modifiers),
                egui::Event::Copy => c(Key::C),
                egui::Event::Cut => c(Key::X),
                egui::Event::Paste(_) => c(Key::V),
                _ => continue,
            };
            if let Some(cmd) = self.lookup(chord) {
                set.add(cmd);
            }
        }
        set
    }
}
