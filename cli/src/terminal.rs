use std::collections::BTreeMap;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use console::{Term, strip_ansi_codes, style, truncate_str};

pub struct Bar {
    prefix: String,
    message: String,
    started: Instant,
}

impl Bar {
    fn new(prefix: String) -> Self {
        Self {
            prefix,
            message: String::new(),
            started: Instant::now(),
        }
    }

    fn elapsed(&self) -> Duration {
        Instant::now().duration_since(self.started)
    }

    fn format(&self, width: usize) -> String {
        let line = format!(
            "{} | {} {}",
            style(format!("{:>5.1}s", self.elapsed().as_secs_f32())).yellow().for_stderr(),
            self.prefix,
            style(&self.message).dim().for_stderr()
        );

        truncate_str(&line, width.saturating_sub(1), "").into_owned()
    }
}

pub struct BarWriter {
    term: Arc<Terminal>,
    id: usize,
    buf: Vec<u8>,
}

impl BarWriter {
    fn publish(&self, line: &[u8]) {
        let text = sanitize(&String::from_utf8_lossy(line));
        if !text.trim().is_empty() {
            self.term.set_bar_message(self.id, text);
        }
    }
}

impl Write for BarWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(data);

        if let Some(last) = self.buf.iter().rposition(is_terminator) {
            let newest = self.buf[..last]
                .split(is_terminator)
                .rev()
                .find(|line| !line.iter().all(u8::is_ascii_whitespace));
            if let Some(line) = newest {
                self.publish(line);
            }
            self.buf.drain(..=last);
        }

        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.buf.is_empty() {
            self.publish(&self.buf);
        }
        Ok(())
    }
}

impl Drop for BarWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

pub struct Terminal {
    bar_id_counter: Mutex<usize>,
    bars: Mutex<BTreeMap<usize, Bar>>,

    logs: Mutex<Vec<String>>,
    footer: Mutex<Option<String>>,
    lines_drawn: Mutex<usize>,
    term: Term,
    is_tty: bool,
}

impl Default for Terminal {
    fn default() -> Self {
        Self::new()
    }
}

impl Terminal {
    pub fn new() -> Self {
        let term = Term::buffered_stderr();
        Self {
            bar_id_counter: Mutex::new(0),
            bars: Mutex::new(BTreeMap::new()),
            logs: Mutex::new(Vec::new()),
            footer: Mutex::new(None),
            lines_drawn: Mutex::new(0),
            is_tty: term.is_term(),
            term,
        }
    }

    pub fn add_bar(&self, prefix: impl Into<String>) -> usize {
        let id = {
            let mut counter = self.bar_id_counter.lock().unwrap();
            *counter += 1;
            *counter - 1
        };

        let mut bars = self.bars.lock().unwrap();
        bars.insert(id, Bar::new(prefix.into()));
        id
    }

    pub fn get_bar_elapsed(&self, id: usize) -> Option<Duration> {
        self.bars.lock().unwrap().get_mut(&id).map(|bar| bar.elapsed())
    }

    pub fn remove_bar(&self, id: usize) {
        self.bars.lock().unwrap().remove(&id);
    }

    pub fn set_bar_message(&self, id: usize, message: impl Into<String>) {
        if let Some(bar) = self.bars.lock().unwrap().get_mut(&id) {
            bar.message = message.into();
        }
    }

    pub fn get_bar_writer(self: &Arc<Self>, id: usize) -> BarWriter {
        BarWriter {
            term: Arc::clone(self),
            id,
            buf: Vec::new(),
        }
    }

    pub fn println(&self, line: impl Into<String>) {
        self.logs.lock().unwrap().push(line.into());
    }

    pub fn set_footer(&self, line: impl Into<String>) {
        *self.footer.lock().unwrap() = Some(line.into());
    }

    pub fn render(&self) -> io::Result<()> {
        let mut lines_drawn = self.lines_drawn.lock().unwrap();
        let logs = std::mem::take(&mut *self.logs.lock().unwrap());

        if !self.is_tty {
            for line in &logs {
                self.term.write_str(line)?;
                self.term.write_str("\n")?;
            }
            return self.term.flush();
        }

        self.term.clear_last_lines(*lines_drawn)?;

        for line in &logs {
            self.term.write_str(line)?;
            self.term.write_str("\n")?;
        }

        let width = self.term.size().1 as usize;

        let bars = self.bars.lock().unwrap();
        for bar in bars.values() {
            self.term.write_str(&bar.format(width))?;
            self.term.write_str("\n")?;
        }
        let mut drawn = bars.len();
        drop(bars);

        let footer = self.footer.lock().unwrap();
        if let Some(footer) = &*footer {
            self.term.write_str(&truncate_str(footer, width.saturating_sub(1), ""))?;
            self.term.write_str("\n")?;
            drawn += 1;
        }
        drop(footer);

        *lines_drawn = drawn;

        self.term.flush()
    }

    pub fn clear(&self) -> Result<(), io::Error> {
        self.bars.lock().unwrap().clear();
        *self.footer.lock().unwrap() = None;
        self.render()
    }

    pub fn spawn_renderer(self: &Arc<Self>, interval: Duration) -> RenderHandle {
        let stop = Arc::new(AtomicBool::new(false));

        let thread = {
            let term = Arc::clone(self);
            let stop = Arc::clone(&stop);

            thread::spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    let _ = term.render();
                    thread::park_timeout(interval);
                }

                let _ = term.clear();
            })
        };

        RenderHandle { stop, thread: Some(thread) }
    }
}

pub struct RenderHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl RenderHandle {
    fn shutdown(&mut self) {
        if let Some(thread) = self.thread.take() {
            self.stop.store(true, Ordering::Release);
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

impl Drop for RenderHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn is_terminator(b: &u8) -> bool {
    *b == b'\n' || *b == b'\r'
}

fn sanitize(str: &str) -> String {
    let str = strip_ansi_codes(str);
    let mut out = String::new();
    for c in str.chars() {
        match c {
            '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}
