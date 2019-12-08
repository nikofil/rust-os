use crate::mem;
use core::fmt;
use lazy_static::lazy_static;
use spin::Mutex;

lazy_static! {
    pub static ref WRITER: Mutex<ScreenWriter> =
        Mutex::new(ScreenWriter::new(mem::PhysAddr::new(0xb8000)));
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

pub fn cls() {
    WRITER.try_lock().map(|mut lock| lock.clear());
}

#[allow(dead_code)]
pub fn set_color(fg: Color, bg: Color, blink: bool) {
    WRITER.try_lock().map(|mut lock| {
        lock.set_fg(fg);
        lock.set_bg(bg);
        lock.set_blink(blink);
    });
}

#[inline(never)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER
        .try_lock()
        .map(|mut lock| lock.write_fmt(args).unwrap());
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ScreenChar {
    chr: u8,
    color: u8,
}

const BUFFER_HEIGHT: usize = 20;
const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct ScreenWriter {
    col: usize,
    row: usize,
    fg_color: Color,
    bg_color: Color,
    blink: bool,
    buffer: &'static mut Buffer,
}

#[allow(dead_code)]
impl ScreenWriter {
    pub fn new(phys_addr: mem::PhysAddr) -> ScreenWriter {
        ScreenWriter {
            col: 0,
            row: BUFFER_HEIGHT - 1,
            fg_color: Color::White,
            bg_color: Color::Black,
            blink: false,
            buffer: unsafe { phys_addr.to_virt().unwrap().to_ref() },
        }
    }

    pub fn write(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            b => {
                let fg_u4 = self.fg_color as u8 & 0b1111;
                let bg_u3 = self.bg_color as u8 & 0b1111;
                let blink = if self.blink { 1 } else { 0 };
                let color_code: u8 = fg_u4 | bg_u3 << 4 | blink << 7;
                self.buffer.chars[self.row][self.col] = ScreenChar {
                    chr: b,
                    color: color_code,
                };
                self.col += 1;
                if self.col == BUFFER_WIDTH {
                    self.new_line();
                }
            }
        }
    }

    pub fn new_line(&mut self) {
        for r in 0..BUFFER_HEIGHT - 1 {
            for c in 0..BUFFER_WIDTH {
                self.buffer.chars[r][c] = self.buffer.chars[r + 1][c];
            }
        }
        self.clear_line(BUFFER_HEIGHT - 1);
        self.col = 0;
    }

    pub fn clear(&mut self) {
        for r in 0..BUFFER_HEIGHT {
            self.clear_line(r);
        }
    }

    pub fn clear_line(&mut self, line: usize) {
        self.buffer.chars[line] = [ScreenChar {
            chr: b' ',
            color: 0,
        }; BUFFER_WIDTH];
    }

    pub fn set_fg(&mut self, color: Color) {
        self.fg_color = color;
    }

    pub fn set_bg(&mut self, color: Color) {
        self.bg_color = color;
    }

    pub fn set_blink(&mut self, blink: bool) {
        self.blink = blink;
    }

    pub fn get_line(&self, line: usize) -> [u8; BUFFER_WIDTH] {
        let mut line_buf = [0 as u8; BUFFER_WIDTH];
        for (i, c) in self.buffer.chars[BUFFER_HEIGHT - 1 - line]
            .iter()
            .enumerate()
        {
            line_buf[i] = c.chr;
        }
        line_buf
    }
}

impl fmt::Write for ScreenWriter {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        s.chars().for_each(|c| self.write(c as u8));
        return Ok(());
    }
}
