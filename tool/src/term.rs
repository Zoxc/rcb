use std::io::{stderr, Write};
use winapi::um::consoleapi::{GetConsoleMode, SetConsoleMode};
use winapi::um::processenv::GetStdHandle;
use winapi::um::winbase::STD_ERROR_HANDLE;
use winapi::um::wincon::ENABLE_VIRTUAL_TERMINAL_PROCESSING;

pub struct View {
    width: usize,
    lines: Vec<usize>,
    line: usize,
    buffer: String,
}

impl View {
    pub fn new() -> View {
        #[cfg(windows)]
        unsafe {
            let stderr = GetStdHandle(STD_ERROR_HANDLE);
            let mut mode = 0;
            GetConsoleMode(stderr, &mut mode);
            SetConsoleMode(stderr, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }

        Self {
            width: term_size::dimensions().unwrap_or((60, 0)).0 as usize,
            lines: Vec::new(),
            line: 0,
            buffer: String::new(),
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn flush(&mut self) {
        let mut stderr = stderr();
        stderr.write(self.buffer.as_bytes()).ok();
        stderr.flush().ok();
        self.buffer = String::new();
    }

    pub fn reset(&mut self) {
        self.flush();
        self.line = 0;
        self.lines.clear();
    }

    pub fn print(&mut self, data: &str, size: usize) {
        self.buffer.push_str(data);
        self.line += size;
    }

    pub fn newline(&mut self) {
        self.buffer.push_str("\n");
        self.lines.push(self.line);
        self.line = 0;
    }

    pub fn rewind(&mut self) {
        let empty = |len| (0..len).map(|_| ' ').collect::<String>();
        self.lines.push(self.line);
        for (i, &line) in self.lines.iter().enumerate().rev() {
            //self.buffer.push_str(&format!("\x1b[{}K", line));
            //self.buffer.push_str("\x1b[1M");
            self.buffer.push_str("\x1b[0K");
            if i != 0 {
                self.buffer.push_str("\x1b[1A");
            }
            /*self.buffer.push_str(&format!("\x1b[{}P", line));
            self.buffer.push_str("\x1b[3P");
            self.buffer.push_str(&empty(line));
            self.buffer.push_str(&format!("\x1b[{}d", line));
            if i != 0 {
            self.buffer.push_str("\x1b[1f");
            }*/
        }
        self.reset();
    }
}

pub trait Viewable {
    fn view(&self, view: &mut View);
}

impl Viewable for String {
    fn view(&self, view: &mut View) {
        (&**self).view(view)
    }
}

impl Viewable for &'_ str {
    fn view(&self, view: &mut View) {
        view.print(self, self.len())
    }
}

impl<T: Fn(&mut View)> Viewable for T {
    fn view(&self, view: &mut View) {
        self(view)
    }
}

#[macro_export]
macro_rules! view {
    ($view:expr $(, $command:expr)* $(,)?) => {{
        $($crate::term::Viewable::view(&$command, $view);)*
    }}
}

pub fn newline() -> impl Fn(&mut View) {
    move |view| view.newline()
}

pub fn default_color() -> impl Fn(&mut View) {
    move |view| view.print("\x1b[0m", 0)
}

pub fn color(r: u8, g: u8, b: u8) -> impl Fn(&mut View) {
    let code = format!("\x1b[38;2;{};{};{}m", r, g, b);
    move |view| view.print(&code, 0)
}

pub fn progress_bar(prefix: &str, current: usize, total: usize) -> impl Fn(&mut View) {
    progress_bar_perc(prefix, (current as f64) / (total as f64))
}

pub fn progress_bar_perc(prefix: &str, current: f64) -> impl Fn(&mut View) {
    let prefix = prefix.to_owned();
    move |view| {
        let len = std::cmp::min(view.width() - prefix.len(), 70);
        let pos = f64::round(current * (len as f64)) as usize;
        let pos = std::cmp::min(pos, len);
        let p: String = (0..pos).map(|_| '#').collect();
        let r: String = (pos..len).map(|_| '-').collect();

        view!(
            view,
            prefix,
            color(137, 114, 186),
            p,
            color(119, 116, 125),
            r,
            default_color(),
            newline()
        );
    }
}
