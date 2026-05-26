use std::io::{self, Write, stdout};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use regex::{Regex, RegexBuilder};
use unicode_width::UnicodeWidthChar;

use crate::highlight::Line;

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, cursor::Hide)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(stdout(), cursor::Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

pub fn run(path: &str, lines: &[Line]) -> io::Result<()> {
    let _guard = TerminalGuard::enter()?;
    let mut pager = Pager::new(path, lines);
    pager.run()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SearchDir {
    Forward,
    Backward,
}

struct SearchState {
    re: Regex,
    dir: SearchDir,
}

enum Mode {
    Normal,
    SearchInput { dir: SearchDir, buffer: String },
    Help,
}

struct Pager<'a> {
    path: &'a str,
    lines: &'a [Line],
    top: usize,
    left: usize,
    count: Option<usize>,
    pending_z: bool,
    mode: Mode,
    search: Option<SearchState>,
    message: Option<String>,
    cols: usize,
    rows: usize,
}

impl<'a> Pager<'a> {
    fn new(path: &'a str, lines: &'a [Line]) -> Self {
        Self {
            path,
            lines,
            top: 0,
            left: 0,
            count: None,
            pending_z: false,
            mode: Mode::Normal,
            search: None,
            message: None,
            cols: 80,
            rows: 24,
        }
    }

    fn body_rows(&self) -> usize {
        self.rows.saturating_sub(1)
    }

    fn max_top(&self) -> usize {
        self.lines.len().saturating_sub(self.body_rows())
    }

    fn at_end(&self) -> bool {
        self.top >= self.max_top()
    }

    fn run(&mut self) -> io::Result<()> {
        loop {
            let (c, r) = terminal::size()?;
            self.cols = c as usize;
            self.rows = r.max(2) as usize;
            if self.top > self.max_top() {
                self.top = self.max_top();
            }
            self.draw()?;
            match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => {
                    if self.handle_key(k) {
                        break;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, k: KeyEvent) -> bool {
        if !matches!(k.code, KeyCode::Char('Z')) {
            self.pending_z = false;
        }
        // Any keypress clears a transient message except while in search input.
        if !matches!(self.mode, Mode::SearchInput { .. }) {
            self.message = None;
        }

        match self.mode {
            Mode::Normal => self.handle_normal(k),
            Mode::SearchInput { .. } => {
                self.handle_search_input(k);
                false
            }
            Mode::Help => {
                self.handle_help(k);
                false
            }
        }
    }

    fn handle_normal(&mut self, k: KeyEvent) -> bool {
        let body = self.body_rows().max(1);
        let max = self.max_top();
        let count = self.count;

        // Digit prefix.
        if let KeyCode::Char(c @ '0'..='9') = k.code {
            if k.modifiers == KeyModifiers::NONE || k.modifiers == KeyModifiers::SHIFT {
                let d = (c as u8 - b'0') as usize;
                if !(d == 0 && self.count.is_none()) {
                    self.count = Some(
                        self.count
                            .unwrap_or(0)
                            .saturating_mul(10)
                            .saturating_add(d),
                    );
                }
                return false;
            }
        }

        let mut consume = true;

        match (k.code, k.modifiers) {
            // ---- Quit
            (KeyCode::Char('q'), KeyModifiers::NONE)
            | (KeyCode::Char('Q'), KeyModifiers::SHIFT) => return true,
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => return true,
            (KeyCode::Char('Z'), _) => {
                if self.pending_z {
                    return true;
                }
                self.pending_z = true;
                consume = false;
            }

            // ---- Forward one line
            (KeyCode::Char('j'), KeyModifiers::NONE)
            | (KeyCode::Down, _)
            | (KeyCode::Enter, _)
            | (KeyCode::Char('e'), KeyModifiers::CONTROL)
            | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                let n = count.unwrap_or(1);
                self.top = self.top.saturating_add(n).min(max);
            }

            // ---- Backward one line
            (KeyCode::Char('k'), KeyModifiers::NONE)
            | (KeyCode::Up, _)
            | (KeyCode::Char('y'), KeyModifiers::CONTROL)
            | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                let n = count.unwrap_or(1);
                self.top = self.top.saturating_sub(n);
            }

            // ---- Forward one window
            (KeyCode::Char(' '), _)
            | (KeyCode::Char('f'), KeyModifiers::NONE)
            | (KeyCode::Char('f'), KeyModifiers::CONTROL)
            | (KeyCode::Char('v'), KeyModifiers::CONTROL)
            | (KeyCode::PageDown, _) => {
                let n = count.unwrap_or(body);
                self.top = self.top.saturating_add(n).min(max);
            }

            // ---- Backward one window
            (KeyCode::Char('b'), KeyModifiers::NONE)
            | (KeyCode::Char('b'), KeyModifiers::CONTROL)
            | (KeyCode::PageUp, _) => {
                let n = count.unwrap_or(body);
                self.top = self.top.saturating_sub(n);
            }

            // ---- Forward half-window
            (KeyCode::Char('d'), KeyModifiers::NONE)
            | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                let n = count.unwrap_or(body / 2).max(1);
                self.top = self.top.saturating_add(n).min(max);
            }

            // ---- Backward half-window
            (KeyCode::Char('u'), KeyModifiers::NONE)
            | (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                let n = count.unwrap_or(body / 2).max(1);
                self.top = self.top.saturating_sub(n);
            }

            // ---- Go to line / top
            (KeyCode::Char('g'), KeyModifiers::NONE)
            | (KeyCode::Char('<'), _)
            | (KeyCode::Home, _) => {
                self.top = count.map(|n| n.saturating_sub(1).min(max)).unwrap_or(0);
            }

            // ---- Go to line / bottom
            (KeyCode::Char('G'), _) | (KeyCode::Char('>'), _) | (KeyCode::End, _) => {
                self.top = count.map(|n| n.saturating_sub(1).min(max)).unwrap_or(max);
            }

            // ---- Percent of file
            (KeyCode::Char('p'), KeyModifiers::NONE) | (KeyCode::Char('%'), _) => {
                let pct = count.unwrap_or(0).min(100);
                let target = self.lines.len().saturating_mul(pct) / 100;
                self.top = target.min(max);
            }

            // ---- Horizontal scroll: half-screen
            (KeyCode::Right, _) => {
                self.left = self.left.saturating_add(self.cols.max(2) / 2);
            }
            (KeyCode::Left, _) => {
                self.left = self.left.saturating_sub(self.cols.max(2) / 2);
            }

            // ---- Search
            (KeyCode::Char('/'), _) => {
                self.mode = Mode::SearchInput {
                    dir: SearchDir::Forward,
                    buffer: String::new(),
                };
                consume = false;
            }
            (KeyCode::Char('?'), _) => {
                self.mode = Mode::SearchInput {
                    dir: SearchDir::Backward,
                    buffer: String::new(),
                };
                consume = false;
            }
            (KeyCode::Char('n'), KeyModifiers::NONE) => self.repeat_search(false),
            (KeyCode::Char('N'), _) => self.repeat_search(true),

            // ---- Repaint (no-op; loop always redraws)
            (KeyCode::Char('r'), _)
            | (KeyCode::Char('R'), _)
            | (KeyCode::Char('l'), KeyModifiers::CONTROL) => {}

            // ---- File info
            (KeyCode::Char('='), _) | (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                self.message = Some(self.info_string());
            }

            // ---- Help
            (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Char('H'), _) => {
                self.mode = Mode::Help;
                consume = false;
            }

            _ => {
                consume = false;
            }
        }

        if consume {
            self.count = None;
        }
        false
    }

    fn handle_search_input(&mut self, k: KeyEvent) {
        let (dir, mut buffer) = match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::SearchInput { dir, buffer } => (dir, buffer),
            other => {
                self.mode = other;
                return;
            }
        };

        match (k.code, k.modifiers) {
            (KeyCode::Enter, _) => {
                if buffer.is_empty() {
                    return;
                }
                let smart_case = buffer.chars().all(|c| !c.is_uppercase());
                match RegexBuilder::new(&buffer)
                    .case_insensitive(smart_case)
                    .build()
                {
                    Ok(re) => {
                        self.search = Some(SearchState { re, dir });
                        self.message = None;
                        self.do_search(dir, false);
                    }
                    Err(e) => {
                        self.message = Some(format!("Invalid regex: {}", e));
                    }
                }
            }
            (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.message = None;
            }
            (KeyCode::Backspace, _) => {
                if buffer.pop().is_none() {
                    self.message = None;
                } else {
                    self.mode = Mode::SearchInput { dir, buffer };
                }
            }
            (KeyCode::Char(c), m) if m == KeyModifiers::NONE || m == KeyModifiers::SHIFT => {
                buffer.push(c);
                self.mode = Mode::SearchInput { dir, buffer };
            }
            _ => {
                self.mode = Mode::SearchInput { dir, buffer };
            }
        }
    }

    fn handle_help(&mut self, k: KeyEvent) {
        if matches!(
            k.code,
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('H')
        ) || (k.code == KeyCode::Char('c') && k.modifiers == KeyModifiers::CONTROL)
        {
            self.mode = Mode::Normal;
        }
    }

    fn repeat_search(&mut self, reverse: bool) {
        let dir = match (self.search.as_ref(), reverse) {
            (Some(s), false) => s.dir,
            (Some(s), true) => match s.dir {
                SearchDir::Forward => SearchDir::Backward,
                SearchDir::Backward => SearchDir::Forward,
            },
            (None, _) => {
                self.message = Some("No previous search".to_string());
                return;
            }
        };
        self.do_search(dir, true);
    }

    fn do_search(&mut self, dir: SearchDir, skip_current: bool) {
        let Some(state) = self.search.as_ref() else {
            return;
        };
        if self.lines.is_empty() {
            self.message = Some("Pattern not found".to_string());
            return;
        }
        let last = self.lines.len() - 1;
        let start = match (dir, skip_current) {
            (SearchDir::Forward, true) => (self.top + 1).min(last),
            (SearchDir::Forward, false) => self.top,
            (SearchDir::Backward, true) => self.top.saturating_sub(1),
            (SearchDir::Backward, false) => self.top,
        };

        let found = match dir {
            SearchDir::Forward => (start..self.lines.len())
                .find(|&i| state.re.is_match(&line_plain(&self.lines[i]))),
            SearchDir::Backward => (0..=start.min(last))
                .rev()
                .find(|&i| state.re.is_match(&line_plain(&self.lines[i]))),
        };

        match found {
            Some(i) => {
                self.top = i.min(self.max_top());
            }
            None => {
                self.message = Some("Pattern not found".to_string());
            }
        }
    }

    fn info_string(&self) -> String {
        let total = self.lines.len();
        let last = (self.top + self.body_rows()).min(total);
        let pct = if total == 0 {
            100
        } else {
            (last * 100 / total).min(100)
        };
        format!(
            "{}  lines {}-{}/{}  {}%",
            self.path,
            self.top + 1,
            last.max(self.top + 1),
            total,
            pct
        )
    }

    fn draw(&self) -> io::Result<()> {
        let mut out = stdout().lock();
        let body = self.body_rows();

        if matches!(self.mode, Mode::Help) {
            self.draw_help(&mut out)?;
            return out.flush();
        }

        for row in 0..body {
            queue!(
                out,
                cursor::MoveTo(0, row as u16),
                Clear(ClearType::CurrentLine)
            )?;
            let idx = self.top + row;
            if idx < self.lines.len() {
                let rendered =
                    render_line(&self.lines[idx], self.left, self.cols, self.search.as_ref());
                out.write_all(rendered.as_bytes())?;
            } else {
                out.write_all(b"\x1b[38;2;90;90;90m~\x1b[0m")?;
            }
        }

        // Status / prompt line.
        queue!(
            out,
            cursor::MoveTo(0, body as u16),
            Clear(ClearType::CurrentLine)
        )?;
        match &self.mode {
            Mode::SearchInput { dir, buffer } => {
                let prompt = if *dir == SearchDir::Forward { '/' } else { '?' };
                out.write_all(format!("{}{}", prompt, buffer).as_bytes())?;
                // Show input cursor.
                queue!(out, cursor::Show)?;
                queue!(
                    out,
                    cursor::MoveTo((buffer.len() + 1).min(self.cols) as u16, body as u16)
                )?;
                out.flush()?;
                return Ok(());
            }
            Mode::Help => unreachable!(),
            Mode::Normal => {
                queue!(out, cursor::Hide)?;
                if let Some(msg) = &self.message {
                    let mut s = msg.clone();
                    truncate_pad(&mut s, self.cols);
                    out.write_all(s.as_bytes())?;
                } else if let Some(c) = self.count {
                    out.write_all(format!(":{}", c).as_bytes())?;
                } else {
                    let mut s = self.status_string();
                    truncate_pad(&mut s, self.cols);
                    out.write_all(b"\x1b[7m")?;
                    out.write_all(s.as_bytes())?;
                    out.write_all(b"\x1b[0m")?;
                }
            }
        }

        out.flush()
    }

    fn status_string(&self) -> String {
        let total = self.lines.len();
        if total == 0 {
            return format!(" {}  (empty)", self.path);
        }
        let last = (self.top + self.body_rows()).min(total);
        let pct = (last * 100 / total).min(100);
        if self.at_end() {
            format!(" {}  (END)  {}/{}  {}%", self.path, last, total, pct)
        } else {
            format!(" {}  {}/{}  {}%", self.path, last, total, pct)
        }
    }

    fn draw_help<W: Write>(&self, out: &mut W) -> io::Result<()> {
        queue!(out, cursor::MoveTo(0, 0), Clear(ClearType::All))?;
        let body = self.body_rows();
        for (i, line) in HELP_TEXT.lines().enumerate() {
            if i >= body {
                break;
            }
            queue!(out, cursor::MoveTo(0, i as u16))?;
            out.write_all(line.as_bytes())?;
        }
        queue!(
            out,
            cursor::MoveTo(0, body as u16),
            Clear(ClearType::CurrentLine)
        )?;
        let mut footer = String::from(" HELP -- press q, h, or Esc to return ");
        truncate_pad(&mut footer, self.cols);
        out.write_all(b"\x1b[7m")?;
        out.write_all(footer.as_bytes())?;
        out.write_all(b"\x1b[0m")?;
        Ok(())
    }
}

const HELP_TEXT: &str = "\
  cless -- a colorised less

  MOVEMENT
    j  DOWN  ENTER  ^E  ^N        forward  one line   ([N] lines)
    k  UP    ^Y  ^P                backward one line   ([N] lines)
    SPACE  f  ^F  ^V  PgDn         forward  one window
    b  ^B  PgUp                    backward one window
    d  ^D                          forward  half-window
    u  ^U                          backward half-window
    g  <  HOME                     go to first line   ([N]g -> line N)
    G  >  END                      go to last line    ([N]G -> line N)
    p  %                           go to [N] percent into file
    LEFT  RIGHT                    half-screen horizontal scroll

  SEARCHING
    /pattern                       search forward
    ?pattern                       search backward
    n                              repeat last search
    N                              repeat in reverse direction
                                   (smart-case: lower-only -> ignore case)

  OTHER
    =  ^G                          show current file info
    r  R  ^L                       repaint screen
    h  H                           this help screen
    q  Q  ZZ  ^C                   quit
";

fn line_plain(line: &Line) -> String {
    let mut s = String::new();
    for (_, t) in &line.spans {
        s.push_str(t);
    }
    s
}

fn truncate_pad(s: &mut String, cols: usize) {
    let mut w = 0usize;
    let mut keep = String::new();
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > cols {
            break;
        }
        keep.push(ch);
        w += cw;
    }
    while w < cols {
        keep.push(' ');
        w += 1;
    }
    *s = keep;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::highlight::{Color, Line, Style};

    fn line(text: &str) -> Line {
        Line {
            spans: vec![(
                Style {
                    foreground: Color { r: 137, g: 180, b: 250 },
                },
                text.to_string(),
            )],
        }
    }

    fn search(pattern: &str) -> SearchState {
        SearchState {
            re: Regex::new(pattern).unwrap(),
            dir: SearchDir::Forward,
        }
    }

    #[test]
    fn highlights_match_with_inverse_sgr() {
        let l = line("fn main() -> ExitCode {");
        let s = search("main");
        let rendered = render_line(&l, 0, 80, Some(&s));
        assert!(
            rendered.contains("\x1b[0;7;38;2;"),
            "expected inverse SGR for match, got {:?}",
            rendered
        );
        assert!(rendered.contains("main"));
    }

    #[test]
    fn no_inverse_when_search_inactive() {
        let l = line("fn main() -> ExitCode {");
        let rendered = render_line(&l, 0, 80, None);
        assert!(!rendered.contains("\x1b[7m"));
        assert!(!rendered.contains("\x1b[0;7;"));
    }

    #[test]
    fn match_at_line_start() {
        let l = line("main main");
        let s = search("main");
        let rendered = render_line(&l, 0, 80, Some(&s));
        // Two matches => at least two inverse SGR sequences.
        let n = rendered.matches("\x1b[0;7;").count();
        assert!(n >= 2, "expected >=2 inverse SGR, got {} in {:?}", n, rendered);
    }
}

fn render_line(line: &Line, left: usize, cols: usize, search: Option<&SearchState>) -> String {
    let plain = line_plain(line);
    let matches: Vec<(usize, usize)> = if let Some(s) = search {
        s.re
            .find_iter(&plain)
            .map(|m| (m.start(), m.end()))
            .filter(|(a, b)| b > a)
            .collect()
    } else {
        Vec::new()
    };
    let in_match = |byte: usize| matches.iter().any(|&(a, b)| byte >= a && byte < b);

    let mut out = String::new();
    let start = left;
    let end = left.saturating_add(cols);
    let mut col: usize = 0;
    let mut byte_pos: usize = 0;
    let mut prev_sgr: Option<String> = None;
    let mut any_emitted = false;

    for (style, text) in &line.spans {
        let fg = style.foreground;
        for ch in text.chars() {
            let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
            let ch_len = ch.len_utf8();
            if col + cw > end {
                col += cw;
                byte_pos += ch_len;
                continue;
            }
            if col >= start && cw > 0 {
                let invert = in_match(byte_pos);
                let sgr = if invert {
                    format!("\x1b[0;7;38;2;{};{};{}m", fg.r, fg.g, fg.b)
                } else {
                    format!("\x1b[0;38;2;{};{};{}m", fg.r, fg.g, fg.b)
                };
                if prev_sgr.as_deref() != Some(sgr.as_str()) {
                    out.push_str(&sgr);
                    prev_sgr = Some(sgr);
                }
                out.push(ch);
                any_emitted = true;
            }
            col += cw;
            byte_pos += ch_len;
        }
    }
    if any_emitted {
        out.push_str("\x1b[0m");
    }
    out
}
