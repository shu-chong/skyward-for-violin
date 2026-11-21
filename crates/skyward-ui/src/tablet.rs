//! A pad's two-column layout on a landscape panel.
//!
//! Some pads keep their panel's scan-out in landscape while the recovery is
//! held portrait. The pages are laid out on a portrait logical canvas with a
//! permanent navigation column on the left and the page on the right; the
//! whole is turned a quarter turn on its way to the panel's framebuffer. A
//! finger is read in the panel's own coordinates and turned back before it
//! is given to a page, so the touchscreen's axis mapping, which the device's
//! configuration has already calibrated, is left exactly as it was.
//!
//! With no rotation and no column the layer is a straight copy: a device
//! that asks for neither pays nothing for this.

use crate::app::{App, Screen, Touch};
use crate::canvas::Canvas;
use crate::color::{mix, Rgba};
use crate::font::Fonts;
use crate::icons::{self, Icon};
use crate::theme::Theme;

/// Which way a portrait logical canvas is turned onto the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    None,
    /// Clockwise: the logical top edge runs down the panel's right edge.
    Clockwise,
    /// Counter-clockwise: the logical top edge runs up the panel's left edge.
    CounterClockwise,
}

impl Rotation {
    pub fn from_degrees(degrees: u32) -> Self {
        match degrees {
            90 => Self::Clockwise,
            270 => Self::CounterClockwise,
            _ => Self::None,
        }
    }

    /// The logical canvas's size for a panel of `w` by `h`.
    pub fn logical(self, w: u32, h: u32) -> (u32, u32) {
        match self {
            Self::None => (w, h),
            Self::Clockwise | Self::CounterClockwise => (h, w),
        }
    }

    /// Logical `x, y` to the panel's `x, y`. `lh` and `lw` are the logical
    /// canvas's height and width, which are the panel's width and height
    /// respectively once it is turned.
    fn place(self, x: u32, y: u32, lw: u32, lh: u32) -> (u32, u32) {
        match self {
            Self::None => (x, y),
            Self::Clockwise => (lh - 1 - y, x),
            Self::CounterClockwise => (y, lw - 1 - x),
        }
    }

    /// The panel's `x, y` back to the logical canvas's `x, y`.
    fn unplace(self, x: f32, y: f32, lw: u32, lh: u32) -> (f32, f32) {
        match self {
            Self::None => (x, y),
            Self::Clockwise => (y, lh as f32 - 1.0 - x),
            Self::CounterClockwise => (lw as f32 - 1.0 - y, x),
        }
    }
}

/// Where a finger went once the layout has had its say.
pub enum Input {
    /// A touch for the page, in the page's own coordinates.
    Detail(Touch),
    /// A section of the navigation column was picked.
    Select(Screen),
    /// A touch that belongs to the navigation column and does no more.
    Sidebar,
}

const BLUE: Rgba = Rgba::rgb(0x34, 0x82, 0xff);
const GREEN: Rgba = Rgba::rgb(0x1d, 0xcd, 0x3a);
const PURPLE: Rgba = Rgba::rgb(0x77, 0x67, 0xf9);
const ORANGE: Rgba = Rgba::rgb(0xff, 0x69, 0x00);
const RED: Rgba = Rgba::rgb(0xfa, 0x38, 0x2e);
const SLATE: Rgba = Rgba::rgb(0x8c, 0x9d, 0xb0);

/// The sections the column keeps on show. The page under each is the same
/// one the home page opens, so picking one asks nothing new of the pages.
const SECTIONS: &[(Screen, [&str; 3], Icon, Rgba)] = &[
    (Screen::Home, ["主页", "主頁", "Home"], Icon::Phone, BLUE),
    (Screen::Search, ["搜索", "搜尋", "Search"], Icon::More, SLATE),
    (Screen::Wlan, ["WLAN", "Wi-Fi", "Wi-Fi"], Icon::Wlan, BLUE),
    (Screen::Install, ["安装", "安裝", "Install"], Icon::Install, GREEN),
    (Screen::Backup, ["备份", "備份", "Backup"], Icon::Backup, BLUE),
    (Screen::Restore, ["恢复", "還原", "Restore"], Icon::Restore, PURPLE),
    (Screen::Mount, ["挂载", "掛載", "Mount"], Icon::Mount, SLATE),
    (Screen::Decrypt, ["解密", "解密", "Decrypt"], Icon::Lock, BLUE),
    (Screen::Wipe, ["清除数据", "清除資料", "Wipe data"], Icon::Wipe, RED),
    (Screen::Advanced, ["高级", "進階", "Advanced"], Icon::More, SLATE),
    (Screen::Settings, ["设置", "設定", "Settings"], Icon::Gear, SLATE),
    (Screen::Reboot, ["重启", "重新啟動", "Reboot"], Icon::Power, ORANGE),
];

/// The portrait canvas, its navigation column, and the turn onto the panel.
pub struct Tablet {
    /// The page's own canvas. Pages compose into this; its size is what
    /// [`App::new`](crate::app::App::new) is told the display is.
    pub detail: Canvas,
    sidebar: Canvas,
    /// The whole logical canvas, before it is turned onto the panel. The
    /// charging screen and the sleep curtain are drawn over this, so they
    /// cover the navigation column and the page together.
    pub whole: Canvas,
    rotation: Rotation,
    /// The navigation column's width in logical pixels; zero keeps one column.
    width: u32,
    /// The logical canvas's width and height.
    lw: u32,
    lh: u32,
    dp: f32,
    hits: Vec<([f32; 4], Screen)>,
    /// The section a finger went down on, and how far it has since moved.
    pick: Option<Screen>,
    start: f32,
    moved: bool,
    /// A gesture keeps the pane it began in, even across the divider.
    in_detail: bool,
}

impl Tablet {
    /// `panel` is the framebuffer's size; `column` is the navigation
    /// column's width in logical pixels, or zero for a single column.
    pub fn new(panel_w: u32, panel_h: u32, rotation: Rotation, column: u32, dp: f32) -> Self {
        let (lw, lh) = rotation.logical(panel_w, panel_h);
        // Leave the page a usable width even if a configuration asks for a
        // very wide column.
        let width = column.min(lw.saturating_sub(320));
        Self {
            detail: Canvas::new(lw - width, lh),
            sidebar: Canvas::new(width, lh),
            whole: Canvas::new(lw, lh),
            rotation,
            width,
            lw,
            lh,
            dp,
            hits: Vec::new(),
            pick: None,
            start: 0.0,
            moved: false,
            in_detail: false,
        }
    }

    /// The size the pages are laid out for.
    pub fn page_size(&self) -> (u32, u32) {
        (self.detail.w, self.detail.h)
    }

    /// Put the navigation column and the page together on the logical
    /// canvas, without turning it. The caller then draws the whole-panel
    /// overlays (charging screen, sleep curtain) onto `whole`, so they reach
    /// both panes at once, before [`Tablet::turn`] puts it on the panel.
    pub fn assemble(&mut self, f: &Fonts, app: &App) {
        if self.width > 0 {
            self.draw_sidebar(f, app);
        }
        let (side, page) = (&self.sidebar, &self.detail);
        let width = self.width as usize;
        let row = self.lw as usize;
        for y in 0..self.lh as usize {
            for x in 0..self.lw as usize {
                let (source, sx) = if x < width { (side, x) } else { (page, x - width) };
                let from = (y * source.w as usize + sx) * 4;
                let to = (y * row + x) * 4;
                self.whole.data[to..to + 4].copy_from_slice(&source.data[from..from + 4]);
            }
        }
    }

    /// Turn the logical whole, overlays and all, onto the panel's canvas.
    pub fn turn(&self, out: &mut Canvas) {
        if self.rotation == Rotation::None {
            out.data.copy_from_slice(&self.whole.data);
            return;
        }
        let row = self.lw as usize;
        let dst_row = out.w as usize;
        for y in 0..self.lh as usize {
            for x in 0..self.lw as usize {
                let from = (y * row + x) * 4;
                let (px, py) = self.rotation.place(x as u32, y as u32, self.lw, self.lh);
                let to = (py as usize * dst_row + px as usize) * 4;
                out.data[to..to + 4].copy_from_slice(&self.whole.data[from..from + 4]);
            }
        }
    }

    /// The whole logical canvas's size, which a full-screen page (the lock)
    /// is laid out for.
    pub fn logical_size(&self) -> (u32, u32) {
        (self.lw, self.lh)
    }

    /// A touch on a full-screen page, turned back into whole-canvas
    /// coordinates: no navigation column, no page offset. The page owns
    /// every pixel, so every touch goes to it.
    pub fn route_whole(&self, t: Touch) -> Touch {
        let unplace = |x: f32, y: f32| self.rotation.unplace(x, y, self.lw, self.lh);
        match t {
            Touch::Down(px, py) => {
                let (x, y) = unplace(px, py);
                Touch::Down(x, y)
            }
            Touch::Move(px, py) => {
                let (x, y) = unplace(px, py);
                Touch::Move(x, y)
            }
            Touch::Up => Touch::Up,
        }
    }

    /// Turn a finger's panel coordinates back to the logical canvas and
    /// give it either to the column or, less its width, to the page.
    pub fn route(&mut self, t: Touch) -> Input {
        let (rotation, lw, lh) = (self.rotation, self.lw, self.lh);
        let unplace = move |x: f32, y: f32| rotation.unplace(x, y, lw, lh);
        match t {
            Touch::Down(px, py) => {
                let (x, y) = unplace(px, py);
                self.in_detail = x >= self.width as f32;
                if self.in_detail {
                    self.pick = None;
                    return Input::Detail(Touch::Down(x - self.width as f32, y));
                }
                self.start = y;
                self.moved = false;
                self.pick = self.hit(x, y);
                Input::Sidebar
            }
            Touch::Move(px, py) => {
                let (x, y) = unplace(px, py);
                if self.in_detail {
                    Input::Detail(Touch::Move(x - self.width as f32, y))
                } else {
                    if (y - self.start).abs() > 12.0 * self.dp || x >= self.width as f32 {
                        self.moved = true;
                    }
                    Input::Sidebar
                }
            }
            Touch::Up => {
                if self.in_detail {
                    return Input::Detail(Touch::Up);
                }
                match if self.moved { None } else { self.pick.take() } {
                    Some(screen) => Input::Select(screen),
                    None => Input::Sidebar,
                }
            }
        }
    }

    fn hit(&self, x: f32, y: f32) -> Option<Screen> {
        self.hits
            .iter()
            .find(|(r, _)| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
            .map(|(_, s)| *s)
    }

    fn draw_sidebar(&mut self, f: &Fonts, app: &App) {
        let d = self.dp;
        let w = self.width;
        let h = self.lh;
        let mut t = if app.state.dark {
            Theme::dark(w, h, d)
        } else {
            Theme::light(w, h, d)
        };
        let c = &mut self.sidebar;
        c.clear(t.pal.bg);
        c.text_top(&f.title, "Skyward", 26.0 * d, 56.0 * d, 28.0 * d, t.pal.text, None);
        c.line(w as f32 - 1.0, 30.0 * d, w as f32 - 1.0, h as f32 - 26.0 * d, 1.0, mix(t.pal.bg, t.pal.subtitle, 0.22));

        let top = 156.0 * d;
        let row = 62.0 * d;
        let gap = 9.0 * d;
        let selected = app.screen();
        self.hits.clear();
        t.m.icon = 26.0 * d;
        for (index, &(screen, title, icon, color)) in SECTIONS.iter().enumerate() {
            let y = top + index as f32 * (row + gap);
            if y + row > h as f32 {
                break;
            }
            let r = [16.0 * d, y, w as f32 - 32.0 * d, row];
            let held = self.pick == Some(screen) && !self.moved;
            let shade = if held {
                mix(t.pal.card, t.pal.subtitle, if t.dark { 0.28 } else { 0.20 })
            } else if screen == selected {
                mix(t.pal.card, t.pal.subtitle, if t.dark { 0.19 } else { 0.12 })
            } else {
                t.pal.card
            };
            c.fill_round_rect(r[0], r[1], r[2], r[3], t.m.card_radius, shade);
            icons::tile(c, &t, r[0] + 13.0 * d, y + (row - t.m.icon) / 2.0, color, icon);
            c.text_vc(&f.body, app.state.lang.pick(title), r[0] + 54.0 * d, y + row / 2.0,
                19.0 * d, t.pal.text, Some(r[2] - 66.0 * d));
            self.hits.push((r, screen));
        }
    }
}
