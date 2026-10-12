//! Exact Wi-Fi editor reducer, keyboard geometry, helper protocol, and renderer.
use super::settings::{DrawOp, Rect};
use super::ui;
use crate::network::NetworkStatus;
pub const SSID_MIN: usize = 1;
pub const SSID_MAX: usize = 32;
pub const PASSWORD_MIN: usize = 8;
pub const PASSWORD_MAX: usize = 63;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Field {
    Ssid,
    Password,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Alphabet,
    Symbols,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    pub open: bool,
    pub ssid: String,
    pub passphrase: String,
    pub field: Field,
    pub mode: Mode,
    pub uppercase: bool,
    pub pressed: Option<Target>,
    pub status: String,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Back,
    Ssid,
    Password,
    Save,
    Mode,
    Shift,
    Space,
    Delete,
    Key(char),
}
impl Default for State {
    fn default() -> Self {
        Self::new("", "", Field::Ssid)
    }
}
impl State {
    pub fn new(ssid: &str, password: &str, field: Field) -> Self {
        Self {
            open: true,
            ssid: ssid.into(),
            passphrase: password.into(),
            field,
            mode: Mode::Alphabet,
            uppercase: false,
            pressed: None,
            status: String::new(),
        }
    }
    fn value(&self) -> &str {
        if self.field == Field::Ssid {
            &self.ssid
        } else {
            &self.passphrase
        }
    }
    fn value_mut(&mut self) -> &mut String {
        if self.field == Field::Ssid {
            &mut self.ssid
        } else {
            &mut self.passphrase
        }
    }
    fn limit(&self) -> usize {
        if self.field == Field::Ssid {
            SSID_MAX
        } else {
            PASSWORD_MAX
        }
    }
}
pub fn tail(v: &str, max: usize) -> String {
    if v.chars().count() <= max {
        return v.into();
    }
    if max <= 3 {
        v.chars()
            .rev()
            .take(max)
            .collect::<String>()
            .chars()
            .rev()
            .collect()
    } else {
        format!(
            "...{}",
            v.chars()
                .rev()
                .take(max - 3)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
        )
    }
}
pub fn rows(mode: Mode) -> &'static [&'static str] {
    match mode {
        Mode::Alphabet => &["qwertyuiop", "asdfghjkl", "zxcvbnm", "@._-"],
        Mode::Symbols => &["1234567890", "!@#$%^&*()", "-_=+[]{}\\|", "`~;:'\",./?<>"],
    }
}
pub fn valid(state: &State) -> Result<(), String> {
    if !(SSID_MIN..=SSID_MAX).contains(&state.ssid.len())
        || !state.ssid.bytes().all(|b| (0x20..=0x7e).contains(&b))
    {
        return Err("SSID MUST BE 1 TO 32 CHARACTERS".into());
    }
    if !(PASSWORD_MIN..=PASSWORD_MAX).contains(&state.passphrase.len())
        || !state.passphrase.bytes().all(|b| (0x20..=0x7e).contains(&b))
    {
        return Err("PASSWORD MUST BE 8 TO 63 CHARACTERS".into());
    }
    Ok(())
}
pub fn apply(s: &mut State, t: Target) {
    let applied = match t {
        Target::Back => {
            s.open = false;
            false
        }
        Target::Ssid => {
            s.field = Field::Ssid;
            true
        }
        Target::Password => {
            s.field = Field::Password;
            true
        }
        Target::Mode => {
            s.mode = if s.mode == Mode::Alphabet {
                Mode::Symbols
            } else {
                Mode::Alphabet
            };
            true
        }
        Target::Shift if s.mode == Mode::Alphabet => {
            s.uppercase = !s.uppercase;
            true
        }
        Target::Shift => false,
        Target::Space => {
            if s.value().chars().count() < s.limit() {
                s.value_mut().push(' ');
            }
            true
        }
        Target::Delete => {
            s.value_mut().pop();
            true
        }
        Target::Key(c) => {
            let upper = s.uppercase;
            if s.value().chars().count() < s.limit() {
                s.value_mut()
                    .push(if upper { c.to_ascii_uppercase() } else { c });
            }
            true
        }
        Target::Save => false,
    };
    if applied {
        s.status.clear();
    }
}
pub fn target_at_mode(mode: Mode, x: i32, y: i32) -> Option<Target> {
    if Rect::new(16, 10, 120, 62).contains(x, y) {
        return Some(Target::Back);
    }
    if Rect::new(330, 10, 310, 62).contains(x, y) {
        return Some(Target::Ssid);
    }
    if Rect::new(650, 10, 330, 62).contains(x, y) {
        return Some(Target::Password);
    }
    if Rect::new(990, 10, 274, 62).contains(x, y) {
        return Some(Target::Save);
    }
    if Rect::new(16, 364, 152, 66).contains(x, y) {
        return Some(Target::Mode);
    }
    if Rect::new(176, 364, 168, 66).contains(x, y) {
        return Some(Target::Shift);
    }
    if Rect::new(352, 364, 700, 66).contains(x, y) {
        return Some(Target::Space);
    }
    if Rect::new(1060, 364, 204, 66).contains(x, y) {
        return Some(Target::Delete);
    }
    let ys = [86, 154, 222, 290];
    for (row, line) in rows(mode).iter().enumerate() {
        let n = line.chars().count() as i32;
        let w = (1280 - 32 - 6 * (n - 1)) / n;
        let left = (1280 - (n * w + 6 * (n - 1))) / 2;
        if y >= ys[row] && y < ys[row] + 62 {
            let i = ((x - left) / (w + 6)) as usize;
            if i < line.chars().count() && x < left + (i as i32) * (w + 6) + w {
                return Some(Target::Key(line.chars().nth(i).unwrap()));
            }
        }
    }
    None
}
pub fn target_at(x: i32, y: i32) -> Option<Target> {
    target_at_mode(Mode::Alphabet, x, y)
}
pub fn touch(s: &mut State, x: i32, y: i32, pressed: bool, released: bool) {
    let t = target_at_mode(s.mode, x, y);
    if pressed {
        s.pressed = t
    }
    if released {
        let old = s.pressed.take();
        if old.is_some() && old == t {
            apply(s, t.unwrap())
        }
    }
}
pub fn helper_input(ssid: &str, password: &str) -> String {
    format!("{ssid}\n{password}\n")
}
pub fn render(s: &State) -> Vec<DrawOp> {
    render_with_network(s, &NetworkStatus::default())
}

pub fn render_with_network(s: &State, network: &NetworkStatus) -> Vec<DrawOp> {
    let mut o = vec![DrawOp::Clear(ui::color("background"))];
    let white = ui::color("white");
    let title = ui::color("title");
    let field = ui::color("field");
    let field_border = ui::color("inactive-border");
    let button_border = ui::color("control-border");
    let focus = ui::color("wifi-focus");
    let surface = ui::color("surface");
    let active = ui::color("wifi-active");
    let active_border = ui::color("wifi-active-border");
    let button = |o: &mut Vec<DrawOp>, r: Rect, label: &str, on: bool| {
        o.push(DrawOp::Fill(r, if on { active } else { surface }));
        o.push(DrawOp::Outline(
            r,
            if on { active_border } else { button_border },
            3,
        ));
        o.push(DrawOp::Centered {
            rect: r,
            text: label.into(),
            scale: ui::fit_text_scale(label, r.w.saturating_sub(12), 3, 1),
            color: white,
        });
    };
    button(&mut o, Rect::new(16, 10, 120, 62), "BACK", false);
    o.push(DrawOp::Text {
        i32_: 158,
        y: 25,
        text: "ADD WIFI".into(),
        scale: 3,
        color: title,
    });
    let ssid = tail(&s.ssid, 19);
    let pass = "*".repeat(s.passphrase.chars().count().min(20));
    for (r, label, value, selected) in [
        (
            Rect::new(330, 10, 310, 62),
            "SSID",
            ssid,
            s.field == Field::Ssid,
        ),
        (
            Rect::new(650, 10, 330, 62),
            "PASSWORD",
            pass,
            s.field == Field::Password,
        ),
    ] {
        o.push(DrawOp::Fill(r, field));
        o.push(DrawOp::Outline(
            r,
            if selected { focus } else { field_border },
            3,
        ));
        o.push(DrawOp::Text {
            i32_: r.x + 10,
            y: r.y + 7,
            text: label.into(),
            scale: 1,
            color: ui::color("field-label"),
        });
        o.push(DrawOp::Text {
            i32_: r.x + 10,
            y: r.y + 28,
            text: value,
            scale: 2,
            color: white,
        });
    }
    button(&mut o, Rect::new(990, 10, 274, 62), "SAVE NETWORK", false);
    for (row, line) in rows(s.mode).iter().enumerate() {
        let n = line.chars().count() as i32;
        let w = (1280 - 32 - 6 * (n - 1)) / n;
        let left = (1280 - (n * w + 6 * (n - 1))) / 2;
        for (i, c) in line.chars().enumerate() {
            let r = Rect::new(
                left + i as i32 * (w + 6),
                86 + row as i32 * 68,
                w as u32,
                62,
            );
            let ch = if s.uppercase && s.mode == Mode::Alphabet {
                c.to_ascii_uppercase()
            } else {
                c
            };
            button(&mut o, r, &ch.to_string(), false);
        }
    }
    button(
        &mut o,
        Rect::new(16, 364, 152, 66),
        if s.mode == Mode::Symbols {
            "ABC"
        } else {
            "123"
        },
        s.mode == Mode::Symbols,
    );
    button(
        &mut o,
        Rect::new(176, 364, 168, 66),
        if s.uppercase { "ABC" } else { "abc" },
        s.uppercase && s.mode == Mode::Alphabet,
    );
    button(&mut o, Rect::new(352, 364, 700, 66), "SPACE", false);
    button(&mut o, Rect::new(1060, 364, 204, 66), "DELETE", false);
    o.push(DrawOp::Centered {
        rect: Rect::new(12, 436, 1256, 10),
        text: if s.status.is_empty() {
            "SAVING DOES NOT INTERRUPT CURRENT WIFI".into()
        } else {
            s.status.clone()
        },
        scale: 1,
        color: ui::color("footer"),
    });
    let addresses = format!(
        "WIFI {}  WLAN0 {}  WG0 {}",
        if network.ssid.is_empty() {
            "NOT CONNECTED"
        } else {
            &network.ssid
        },
        if network.wlan_ipv4.is_empty() {
            "NO ADDRESS"
        } else {
            &network.wlan_ipv4
        },
        if network.wireguard_ipv4.is_empty() {
            "NO ADDRESS"
        } else {
            &network.wireguard_ipv4
        },
    );
    o.push(DrawOp::Centered {
        rect: Rect::new(12, 450, 1256, 10),
        text: ui::fit_text_width(&addresses, 1248, 1),
        scale: 1,
        color: ui::color("text"),
    });
    o.push(DrawOp::Centered {
        rect: Rect::new(12, 464, 1256, 10),
        text: ui::fit_text_width(&format!("AUTO WIFI: {}", network.selector), 1248, 1),
        scale: 1,
        color: ui::color("muted"),
    });
    o
}

pub fn open(s: &mut State) {
    s.open = true;
    s.status.clear();
    s.pressed = None;
}
pub fn target_at_state(s: &State, x: i32, y: i32) -> Option<Target> {
    target_at_mode(s.mode, x, y)
}
pub fn complete_save(s: &mut State, succeeded: bool, failure_status: Option<&str>) {
    if succeeded {
        s.passphrase.clear();
        s.status = "WIFI SAVED - USED AFTER CURRENT WIFI DISCONNECTS".into();
    } else {
        s.status = failure_status
            .unwrap_or("WIFI PROFILE WAS NOT SAVED")
            .into();
    }
}
pub fn render_frame(s: &State, network: &NetworkStatus) -> super::settings::Layout {
    let ops = render_with_network(s, network);
    super::settings::execute_draw_ops(&ops);
    super::settings::Layout::default()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn limits() {
        let mut s = State::new("a", "12345678", Field::Ssid);
        assert!(valid(&s).is_ok());
        s.ssid = "".into();
        assert!(valid(&s).is_err())
    }
    #[test]
    fn keys() {
        let mut s = State::default();
        apply(&mut s, Target::Key('a'));
        assert_eq!(s.ssid, "a");
        apply(&mut s, Target::Shift);
        apply(&mut s, Target::Key('b'));
        assert_eq!(s.ssid, "aB")
    }
    #[test]
    fn helper() {
        assert_eq!(helper_input("a", "b"), "a\nb\n")
    }
}
