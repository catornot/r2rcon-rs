use std::sync::mpsc::Receiver;

use crate::rcon::MAX_CONTENT_SIZE;

pub struct ConsoleAccess {
    console_recv: Receiver<String>,
    cmd_buffer: Vec<String>,
}

impl ConsoleAccess {
    pub fn new(recv: Receiver<String>) -> Self {
        Self {
            console_recv: recv,
            cmd_buffer: Vec::new(),
        }
    }

    pub fn next_line(&self) -> Option<String> {
        self.console_recv.try_recv().ok()
    }

    pub fn next_line_catpure(&mut self) -> Option<String> {
        if let Some(line) = self.next_line() {
            let line_size = line.len();
            let mut buffer_size = 0;

            for bline in self.cmd_buffer.drain(..).rev().collect::<Vec<String>>() {
                buffer_size += bline.len();
                if buffer_size + line_size > MAX_CONTENT_SIZE {
                    break;
                }

                self.cmd_buffer.insert(0, bline);
            }
            self.cmd_buffer.push(line.clone());

            return Some(line);
        }
        None
    }

    pub fn get_last_console_output(&self) -> &[String] {
        &self.cmd_buffer
    }
}
