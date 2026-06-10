use std::collections::HashMap;
use std::fs;
use std::io::{self, Write, stdout};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use regex::{Regex, RegexBuilder};
use unicode_width::UnicodeWidthChar;

use crate::highlight::{Line, highlight_file};

/// An input the pager can display. Files are read and highlighted lazily on
/// switch; stdin content is pre-highlighted (it cannot be re-read).
pub struct Source {
    name: String,
    kind: SourceKind,
}

enum SourceKind {
    File(String),
    Memory(Vec<Line>),
}

impl Source {
    pub fn file(name: impl Into<String>, path: impl Into<String>) -> Self {
        Source {
            name: name.into(),
            kind: SourceKind::File(path.into()),
        }
    }

    pub fn memory(name: impl Into<String>, lines: Vec<Line>) -> Self {
        Source {
            name: name.into(),
            kind: SourceKind::Memory(lines),
        }
    }
}

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

pub fn run(sources: Vec<Source>, wrap: bool, numbers: bool) -> io::Result<()> {
    let _guard = TerminalGuard::enter()?;
    let mut pager = Pager::new(sources, wrap, numbers)?;
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

/// A keystroke that expects a follow-up key. Replaces a pile of parallel
/// booleans so two prefixes can never be live at once.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    None,
    Z,     // saw `Z`, waiting for a second `Z` to quit (ZZ)
    Dash,  // saw `-`, waiting for an option letter (S/N)
    Mark,  // saw `m`, waiting for a letter to name the mark
    Quote, // saw `'`, waiting for a letter to jump to a mark
    Colon, // saw `:`, waiting for a file command (n/p)
}

struct Pager {
    sources: Vec<Source>,
    index: usize,
    name: String,
    lines: Vec<Line>,
    // Scroll position: first visible source line (`top`) and, when wrapping,
    // the wrap-segment within it shown at the top of the screen (`sub`).
    top: usize,
    sub: usize,
    left: usize,
    wrap: bool,
    numbers: bool,
    count: Option<usize>,
    pending: Pending,
    marks: HashMap<char, (usize, usize)>,
    prev_pos: Option<(usize, usize)>,
    mode: Mode,
    search: Option<SearchState>,
    message: Option<String>,
    cols: usize,
    rows: usize,
}

impl Pager {
    fn new(sources: Vec<Source>, wrap: bool, numbers: bool) -> io::Result<Self> {
        let mut pager = Self {
            sources,
            index: 0,
            name: String::new(),
            lines: Vec::new(),
            top: 0,
            sub: 0,
            left: 0,
            wrap,
            numbers,
            count: None,
            pending: Pending::None,
            marks: HashMap::new(),
            prev_pos: None,
            mode: Mode::Normal,
            search: None,
            message: None,
            cols: 80,
            rows: 24,
        };
        pager.load(0)?;
        Ok(pager)
    }

    /// Read and highlight source `index`, resetting view state and marks.
    /// Errors propagate (the initial load aborts); switches handle them.
    fn load(&mut self, index: usize) -> io::Result<()> {
        let lines = match &self.sources[index].kind {
            SourceKind::Memory(l) => l.to_vec(),
            SourceKind::File(path) => {
                let content = fs::read_to_string(path)?;
                highlight_file(&content, path)
            }
        };
        self.index = index;
        self.name = self.sources[index].name.clone();
        self.lines = lines;
        self.top = 0;
        self.sub = 0;
        self.left = 0;
        self.marks.clear();
        self.prev_pos = None;
        Ok(())
    }

    fn switch_file(&mut self, index: usize) {
        match self.load(index) {
            // read_to_string fails before mutating self, so state stays valid.
            Err(e) => self.message = Some(format!("{}: {}", self.sources[index].name, e)),
            Ok(()) => {
                self.message =
                    Some(format!("{} (file {}/{})", self.name, index + 1, self.sources.len()))
            }
        }
    }

    fn next_file(&mut self) {
        if self.index + 1 < self.sources.len() {
            self.switch_file(self.index + 1);
        } else {
            self.message = Some("No next file".to_string());
        }
    }

    fn prev_file(&mut self) {
        if self.index > 0 {
            self.switch_file(self.index - 1);
        } else {
            self.message = Some("No previous file".to_string());
        }
    }

    fn body_rows(&self) -> usize {
        self.rows.saturating_sub(1)
    }

    /// Width of the line-number gutter (0 when line numbers are off).
    fn gutter_width(&self) -> usize {
        gutter_width(self.lines.len(), self.numbers)
    }

    /// Columns available for line content after the gutter. Wrapping and all
    /// position math use this, so wrapped lines fit beside the numbers.
    fn content_cols(&self) -> usize {
        self.cols.saturating_sub(self.gutter_width()).max(1)
    }

    /// Number of screen rows the given source line occupies (always >= 1).
    fn line_height(&self, idx: usize) -> usize {
        line_height_of(&self.lines[idx], self.content_cols(), self.wrap)
    }

    /// Last display row of the file: (last line, its last wrap segment).
    fn end_pos(&self) -> (usize, usize) {
        if self.lines.is_empty() {
            return (0, 0);
        }
        let last = self.lines.len() - 1;
        (last, self.line_height(last).saturating_sub(1))
    }

    /// Scroll position that puts the final screenful at the bottom — the wrap
    /// analog of the old `max_top`. Short files stay at the top.
    fn max_scroll(&self) -> (usize, usize) {
        let body = self.body_rows().max(1);
        let (t, s) = self.end_pos();
        self.pos_up(t, s, body - 1)
    }

    /// Move a position down by `n` display rows, stopping at `end_pos`.
    fn pos_down(&self, mut top: usize, mut sub: usize, n: usize) -> (usize, usize) {
        for _ in 0..n {
            if top >= self.lines.len() {
                break;
            }
            if sub + 1 < self.line_height(top) {
                sub += 1;
            } else if top + 1 < self.lines.len() {
                top += 1;
                sub = 0;
            } else {
                break;
            }
        }
        (top, sub)
    }

    /// Move a position up by `n` display rows, stopping at the very top.
    fn pos_up(&self, mut top: usize, mut sub: usize, n: usize) -> (usize, usize) {
        for _ in 0..n {
            if sub > 0 {
                sub -= 1;
            } else if top > 0 {
                top -= 1;
                sub = self.line_height(top).saturating_sub(1);
            } else {
                break;
            }
        }
        (top, sub)
    }

    fn scroll_down(&mut self, n: usize) {
        let p = self.pos_down(self.top, self.sub, n).min(self.max_scroll());
        (self.top, self.sub) = p;
    }

    fn scroll_up(&mut self, n: usize) {
        (self.top, self.sub) = self.pos_up(self.top, self.sub, n);
    }

    /// Jump to the top of source line `line`, clamped into range.
    fn goto_line(&mut self, line: usize) {
        let p = clamp_pos(&self.lines, self.content_cols(), self.wrap, line, 0).min(self.max_scroll());
        (self.top, self.sub) = p;
    }

    fn goto_end(&mut self) {
        (self.top, self.sub) = self.max_scroll();
    }

    /// Jump to a saved position, remembering the current one for `''`.
    fn jump_to(&mut self, pos: (usize, usize)) {
        let cur = (self.top, self.sub);
        let p = clamp_pos(&self.lines, self.content_cols(), self.wrap, pos.0, pos.1)
            .min(self.max_scroll());
        self.prev_pos = Some(cur);
        (self.top, self.sub) = p;
    }

    fn goto_mark(&mut self, c: char) {
        // `''` returns to the position before the last jump.
        let target = if c == '\'' {
            self.prev_pos
        } else {
            self.marks.get(&c).copied()
        };
        match target {
            Some(pos) => self.jump_to(pos),
            None => self.message = Some("Mark not set".to_string()),
        }
    }

    /// Source line index shown on the bottom body row.
    fn bottom_line(&self) -> usize {
        let body = self.body_rows().max(1);
        self.pos_down(self.top, self.sub, body - 1).0
    }

    fn at_end(&self) -> bool {
        (self.top, self.sub) >= self.max_scroll()
    }

    fn toggle_wrap(&mut self) {
        self.wrap = !self.wrap;
        self.left = 0;
        // Keep the current line in view; drop the sub-row offset.
        let p = clamp_pos(&self.lines, self.content_cols(), self.wrap, self.top, 0).min(self.max_scroll());
        (self.top, self.sub) = p;
        self.message = Some(
            if self.wrap {
                "Wrap long lines"
            } else {
                "Chop long lines"
            }
            .to_string(),
        );
    }

    fn toggle_numbers(&mut self) {
        self.numbers = !self.numbers;
        // Gutter changes the content width, so re-clamp the position.
        let p = clamp_pos(&self.lines, self.content_cols(), self.wrap, self.top, self.sub)
            .min(self.max_scroll());
        (self.top, self.sub) = p;
        self.message = Some(
            if self.numbers {
                "Line numbers"
            } else {
                "No line numbers"
            }
            .to_string(),
        );
    }

    fn run(&mut self) -> io::Result<()> {
        loop {
            let (c, r) = terminal::size()?;
            self.cols = c as usize;
            self.rows = r.max(2) as usize;
            // Resize may shrink a line's wrap-segment count; guard `sub` first
            // (avoids indexing past the end), then keep the end on-screen.
            (self.top, self.sub) =
                clamp_pos(&self.lines, self.content_cols(), self.wrap, self.top, self.sub);
            let max = self.max_scroll();
            if (self.top, self.sub) > max {
                (self.top, self.sub) = max;
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
        let count = self.count;

        // Resolve a pending prefix key, if any. Unhandled combinations fall
        // through so the second key is processed normally (matches less, e.g.
        // `Z` then a non-`Z` key).
        let pending = std::mem::replace(&mut self.pending, Pending::None);
        match (pending, k.code) {
            // `-S` chop long lines, `-N` line numbers.
            (Pending::Dash, KeyCode::Char('S')) => {
                self.toggle_wrap();
                self.count = None;
                return false;
            }
            (Pending::Dash, KeyCode::Char('N')) => {
                self.toggle_numbers();
                self.count = None;
                return false;
            }
            // `m<char>` sets a mark at the current position.
            (Pending::Mark, KeyCode::Char(c)) if c.is_ascii_alphanumeric() => {
                self.marks.insert(c, (self.top, self.sub));
                self.count = None;
                return false;
            }
            // `'<char>` jumps to a mark; `''` returns to the previous position.
            (Pending::Quote, KeyCode::Char(c)) => {
                self.goto_mark(c);
                self.count = None;
                return false;
            }
            // `:n` / `:p` switch between multiple files.
            (Pending::Colon, KeyCode::Char('n')) => {
                self.next_file();
                self.count = None;
                return false;
            }
            (Pending::Colon, KeyCode::Char('p')) => {
                self.prev_file();
                self.count = None;
                return false;
            }
            _ => {}
        }

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
                if pending == Pending::Z {
                    return true;
                }
                self.pending = Pending::Z;
                consume = false;
            }

            // ---- Option toggle prefix (`-S` chops, `-N` numbers)
            (KeyCode::Char('-'), _) => {
                self.pending = Pending::Dash;
                consume = false;
            }

            // ---- Marks: `m<char>` set, `'<char>` jump, `''` previous position
            (KeyCode::Char('m'), KeyModifiers::NONE) => {
                self.pending = Pending::Mark;
                consume = false;
            }
            (KeyCode::Char('\''), _) => {
                self.pending = Pending::Quote;
                consume = false;
            }

            // ---- Multiple files: `:n` next, `:p` previous
            (KeyCode::Char(':'), _) => {
                self.pending = Pending::Colon;
                consume = false;
            }

            // ---- Forward one line
            (KeyCode::Char('j'), KeyModifiers::NONE)
            | (KeyCode::Down, _)
            | (KeyCode::Enter, _)
            | (KeyCode::Char('e'), KeyModifiers::CONTROL)
            | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                self.scroll_down(count.unwrap_or(1));
            }

            // ---- Backward one line
            (KeyCode::Char('k'), KeyModifiers::NONE)
            | (KeyCode::Up, _)
            | (KeyCode::Char('y'), KeyModifiers::CONTROL)
            | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                self.scroll_up(count.unwrap_or(1));
            }

            // ---- Forward one window
            (KeyCode::Char(' '), _)
            | (KeyCode::Char('f'), KeyModifiers::NONE)
            | (KeyCode::Char('f'), KeyModifiers::CONTROL)
            | (KeyCode::Char('v'), KeyModifiers::CONTROL)
            | (KeyCode::PageDown, _) => {
                self.scroll_down(count.unwrap_or(body));
            }

            // ---- Backward one window
            (KeyCode::Char('b'), KeyModifiers::NONE)
            | (KeyCode::Char('b'), KeyModifiers::CONTROL)
            | (KeyCode::PageUp, _) => {
                self.scroll_up(count.unwrap_or(body));
            }

            // ---- Forward half-window
            (KeyCode::Char('d'), KeyModifiers::NONE)
            | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                self.scroll_down(count.unwrap_or(body / 2).max(1));
            }

            // ---- Backward half-window
            (KeyCode::Char('u'), KeyModifiers::NONE)
            | (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.scroll_up(count.unwrap_or(body / 2).max(1));
            }

            // ---- Go to line / top
            (KeyCode::Char('g'), KeyModifiers::NONE)
            | (KeyCode::Char('<'), _)
            | (KeyCode::Home, _) => {
                self.goto_line(count.map(|n| n.saturating_sub(1)).unwrap_or(0));
            }

            // ---- Go to line / bottom
            (KeyCode::Char('G'), _) | (KeyCode::Char('>'), _) | (KeyCode::End, _) => {
                match count {
                    Some(n) => self.goto_line(n.saturating_sub(1)),
                    None => self.goto_end(),
                }
            }

            // ---- Percent of file
            (KeyCode::Char('p'), KeyModifiers::NONE) | (KeyCode::Char('%'), _) => {
                let pct = count.unwrap_or(0).min(100);
                let target = self.lines.len().saturating_mul(pct) / 100;
                self.goto_line(target);
            }

            // ---- Horizontal scroll: half-screen (chop mode only; wrapping has no left)
            (KeyCode::Right, _) => {
                if !self.wrap {
                    self.left = self.left.saturating_add(self.cols.max(2) / 2);
                }
            }
            (KeyCode::Left, _) => {
                if !self.wrap {
                    self.left = self.left.saturating_sub(self.cols.max(2) / 2);
                }
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
                self.goto_line(i);
            }
            None => {
                self.message = Some("Pattern not found".to_string());
            }
        }
    }

    fn info_string(&self) -> String {
        let total = self.lines.len();
        let last = (self.bottom_line() + 1).min(total);
        let pct = if total == 0 {
            100
        } else {
            (last * 100 / total).min(100)
        };
        format!(
            "{}  lines {}-{}/{}  {}%",
            self.name,
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

        let cols = self.content_cols();
        let digits = self.gutter_width().saturating_sub(1);
        let mut top = self.top;
        let mut sub = self.sub;
        // Wrap-segment byte ranges for the current source line (wrap mode only).
        let mut ranges = if self.wrap && top < self.lines.len() {
            wrap_ranges(&self.lines[top], cols)
        } else {
            Vec::new()
        };
        for row in 0..body {
            queue!(
                out,
                cursor::MoveTo(0, row as u16),
                Clear(ClearType::CurrentLine)
            )?;
            if top < self.lines.len() {
                // Line number prints on a line's first display row only;
                // wrapped continuation rows get a blank gutter of equal width.
                if self.numbers {
                    out.write_all(gutter(top + 1, sub == 0, digits).as_bytes())?;
                }
                if self.wrap {
                    let range = ranges[sub.min(ranges.len() - 1)];
                    let rendered =
                        render_segment(&self.lines[top], range, self.search.as_ref());
                    out.write_all(rendered.as_bytes())?;
                    if sub + 1 < ranges.len() {
                        sub += 1;
                    } else {
                        top += 1;
                        sub = 0;
                        if top < self.lines.len() {
                            ranges = wrap_ranges(&self.lines[top], cols);
                        }
                    }
                } else {
                    let rendered =
                        render_line(&self.lines[top], self.left, cols, self.search.as_ref());
                    out.write_all(rendered.as_bytes())?;
                    top += 1;
                }
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
        let files = if self.sources.len() > 1 {
            format!(" (file {}/{})", self.index + 1, self.sources.len())
        } else {
            String::new()
        };
        let total = self.lines.len();
        if total == 0 {
            return format!(" {}{}  (empty)", self.name, files);
        }
        let last = (self.bottom_line() + 1).min(total);
        let pct = (last * 100 / total).min(100);
        if self.at_end() {
            format!(" {}{}  (END)  {}/{}  {}%", self.name, files, last, total, pct)
        } else {
            format!(" {}{}  {}/{}  {}%", self.name, files, last, total, pct)
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
    LEFT  RIGHT                    half-screen horizontal scroll (chop mode)

  MARKS
    m<letter>                      set mark at current position
    '<letter>                      jump to mark
    ''                             jump to previous position

  FILES
    :n                             next file
    :p                             previous file

  SEARCHING
    /pattern                       search forward
    ?pattern                       search backward
    n                              repeat last search
    N                              repeat in reverse direction
                                   (smart-case: lower-only -> ignore case)

  OTHER
    -S                             toggle wrap / chop long lines
    -N                             toggle line numbers
    =  ^G                          show current file info
    r  R  ^L                       repaint screen
    h  H                           this help screen
    q  Q  ZZ  ^C                   quit
";

/// Byte ranges `[start, end)` (over the concatenated span text) of each wrap
/// segment for a line at the given width. The single source of truth for where
/// wrapping breaks — `render_segment` renders these ranges and never re-decides.
/// Always returns at least one range, so empty lines yield `[(0, 0)]`.
fn wrap_ranges(line: &Line, cols: usize) -> Vec<(usize, usize)> {
    let cols = cols.max(1);
    let mut ranges = Vec::new();
    let mut seg_start = 0usize;
    let mut byte_pos = 0usize;
    let mut col = 0usize;
    for (_, text) in &line.spans {
        for ch in text.chars() {
            let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
            // Break before a char that no longer fits, unless the row is empty
            // (a single char wider than the screen still gets its own row).
            if cw > 0 && col + cw > cols && col > 0 {
                ranges.push((seg_start, byte_pos));
                seg_start = byte_pos;
                col = 0;
            }
            col += cw;
            byte_pos += ch.len_utf8();
        }
    }
    ranges.push((seg_start, byte_pos));
    ranges
}

/// Decimal digits in `n` (at least 1, so 0 -> 1).
fn digit_count(n: usize) -> usize {
    let mut n = n;
    let mut d = 1;
    while n >= 10 {
        n /= 10;
        d += 1;
    }
    d
}

/// Width of the line-number gutter: digits of the largest line number plus a
/// one-column separator, or 0 when line numbers are disabled.
fn gutter_width(n_lines: usize, numbers: bool) -> usize {
    if numbers {
        digit_count(n_lines.max(1)) + 1
    } else {
        0
    }
}

/// The gutter cell for a display row: the right-aligned line number on a line's
/// first row, or blanks (same width) on a wrapped continuation row.
fn gutter(line_no: usize, first_row: bool, digits: usize) -> String {
    if first_row {
        format!("\x1b[38;2;90;90;90m{:>w$} \x1b[0m", line_no, w = digits)
    } else {
        " ".repeat(digits + 1)
    }
}

/// Screen rows a line occupies: 1 in chop mode, else its wrap-segment count.
fn line_height_of(line: &Line, cols: usize, wrap: bool) -> usize {
    if wrap {
        wrap_ranges(line, cols).len()
    } else {
        1
    }
}

/// Clamp a scroll position into range: a valid line and a valid wrap-segment
/// within it. Guards against `sub` pointing past a line's segments after a
/// resize widens the screen (which would otherwise index out of bounds).
fn clamp_pos(
    lines: &[Line],
    cols: usize,
    wrap: bool,
    mut top: usize,
    mut sub: usize,
) -> (usize, usize) {
    if lines.is_empty() {
        return (0, 0);
    }
    if top >= lines.len() {
        top = lines.len() - 1;
    }
    let h = line_height_of(&lines[top], cols, wrap);
    if sub >= h {
        sub = h - 1;
    }
    (top, sub)
}

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

    #[test]
    fn wrap_exact_fit_is_one_segment() {
        let l = line("abcde"); // width 5
        assert_eq!(wrap_ranges(&l, 5), vec![(0, 5)]);
    }

    #[test]
    fn wrap_overflow_breaks() {
        let l = line("abcdef"); // width 6, cols 5 -> "abcde" + "f"
        assert_eq!(wrap_ranges(&l, 5), vec![(0, 5), (5, 6)]);
    }

    #[test]
    fn wrap_empty_line_is_single_segment() {
        let l = line("");
        assert_eq!(wrap_ranges(&l, 5), vec![(0, 0)]);
    }

    #[test]
    fn wrap_wide_char_straddling_boundary() {
        // "aあb": 'a'=w1/1B, 'あ'=w2/3B, 'b'=w1/1B. cols=2.
        // row0: 'a' (col1), 'あ' would make col3>2 -> break -> (0,1).
        // row1: 'あ' (col2), 'b' would make col3>2 -> break -> (1,4).
        // row2: 'b' -> (4,5).
        let l = line("aあb");
        assert_eq!(wrap_ranges(&l, 2), vec![(0, 1), (1, 4), (4, 5)]);
    }

    #[test]
    fn wrap_zero_width_char_stays_in_segment() {
        // Tabs report width 0 and must not force a wrap break.
        let l = line("a\tb"); // widths 1,0,1 -> fits in cols 2 as one segment
        assert_eq!(wrap_ranges(&l, 2), vec![(0, 3)]);
    }

    #[test]
    fn wrap_char_wider_than_screen_gets_own_row() {
        // cols=1 but 'あ' is width 2: it occupies its own row rather than
        // producing an empty leading segment.
        let l = line("あい");
        assert_eq!(wrap_ranges(&l, 1), vec![(0, 3), (3, 6)]);
    }

    #[test]
    fn clamp_pos_guards_sub_overflow() {
        // Line wraps to 2 segments at cols 5; a stale sub=9 (e.g. after a
        // resize widened the screen) clamps to the last valid segment.
        let lines = vec![line("abcdef")];
        assert_eq!(clamp_pos(&lines, 5, true, 0, 9), (0, 1));
        // After widening to cols 10 it is a single segment: sub clamps to 0.
        assert_eq!(clamp_pos(&lines, 10, true, 0, 9), (0, 0));
    }

    #[test]
    fn clamp_pos_chop_mode_sub_is_zero() {
        let lines = vec![line("abcdef")];
        assert_eq!(clamp_pos(&lines, 5, false, 0, 3), (0, 0));
    }

    #[test]
    fn clamp_pos_clamps_top_to_last_line() {
        let lines = vec![line("a"), line("b")];
        assert_eq!(clamp_pos(&lines, 5, true, 9, 0), (1, 0));
    }

    #[test]
    fn clamp_pos_empty_file() {
        let lines: Vec<Line> = Vec::new();
        assert_eq!(clamp_pos(&lines, 5, true, 3, 2), (0, 0));
    }

    #[test]
    fn digit_count_boundaries() {
        assert_eq!(digit_count(0), 1);
        assert_eq!(digit_count(9), 1);
        assert_eq!(digit_count(10), 2);
        assert_eq!(digit_count(99), 2);
        assert_eq!(digit_count(100), 3);
    }

    #[test]
    fn gutter_width_tracks_line_count() {
        assert_eq!(gutter_width(9, true), 2); // 1 digit + separator
        assert_eq!(gutter_width(10, true), 3); // 2 digits + separator
        assert_eq!(gutter_width(1000, true), 5); // 4 digits + separator
        assert_eq!(gutter_width(1000, false), 0); // disabled
    }

    #[test]
    fn line_numbers_narrow_the_wrap_width() {
        // 6-wide content; at cols=8 with a 3-col gutter (lines<=99) the usable
        // width is 5, so the line wraps into two segments.
        let l = line("abcdef");
        let content_cols = 8 - gutter_width(50, true); // 8 - 3 = 5
        assert_eq!(content_cols, 5);
        assert_eq!(wrap_ranges(&l, content_cols), vec![(0, 5), (5, 6)]);
    }

    #[test]
    fn gutter_blank_on_continuation_rows() {
        let first = gutter(42, true, 3);
        let cont = gutter(42, false, 3);
        assert!(first.contains("42"));
        assert_eq!(cont, "    "); // 3 digits + separator, all spaces
        assert!(!cont.contains("42"));
    }

    fn pager_with(n: usize) -> Pager {
        let lines: Vec<Line> = (0..n).map(|i| line(&format!("line {}", i))).collect();
        let mut p = Pager::new(vec![Source::memory("t", lines)], false, false).unwrap();
        p.cols = 80;
        p.rows = 24;
        p
    }

    #[test]
    fn mark_set_and_jump() {
        let mut p = pager_with(100);
        p.goto_line(50);
        p.marks.insert('a', (p.top, p.sub));
        p.goto_line(10);
        assert_eq!(p.top, 10);
        p.goto_mark('a');
        assert_eq!(p.top, 50);
    }

    #[test]
    fn quote_quote_returns_to_previous_position() {
        let mut p = pager_with(100);
        p.goto_line(10);
        p.marks.insert('a', (50, 0));
        p.goto_mark('a'); // jump to 50, remembering 10
        assert_eq!(p.top, 50);
        p.goto_mark('\''); // '' back to 10
        assert_eq!(p.top, 10);
    }

    #[test]
    fn jump_to_unset_mark_reports_and_stays() {
        let mut p = pager_with(100);
        p.goto_mark('z');
        assert!(p.message.is_some());
        assert_eq!(p.top, 0);
    }

    #[test]
    fn switch_between_files_resets_view() {
        let a: Vec<Line> = (0..10).map(|i| line(&format!("a{}", i))).collect();
        let b: Vec<Line> = (0..5).map(|i| line(&format!("b{}", i))).collect();
        let mut p =
            Pager::new(vec![Source::memory("a", a), Source::memory("b", b)], false, false).unwrap();
        p.cols = 80;
        p.rows = 24;
        assert_eq!((p.index, p.lines.len()), (0, 10));

        p.goto_line(5);
        p.next_file();
        assert_eq!((p.index, p.lines.len()), (1, 5));
        assert_eq!(p.top, 0); // view reset on switch

        p.next_file(); // already last
        assert_eq!(p.index, 1);

        p.prev_file();
        assert_eq!((p.index, p.lines.len()), (0, 10));
    }
}

/// Render one wrap segment (a byte range from `wrap_ranges`) of a line, with
/// syntax colors and search-match inverse video, terminated by a reset.
fn render_segment(line: &Line, range: (usize, usize), search: Option<&SearchState>) -> String {
    let (start, end) = range;
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
    let mut byte_pos: usize = 0;
    let mut prev_sgr: Option<String> = None;
    let mut any_emitted = false;

    for (style, text) in &line.spans {
        let fg = style.foreground;
        for ch in text.chars() {
            let ch_len = ch.len_utf8();
            if byte_pos >= start && byte_pos < end && UnicodeWidthChar::width(ch).unwrap_or(0) > 0 {
                let sgr = if in_match(byte_pos) {
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
            byte_pos += ch_len;
        }
    }
    if any_emitted {
        out.push_str("\x1b[0m");
    }
    out
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
