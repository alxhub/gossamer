use super::buffer::*;
use std::io::Write;

pub struct LineCodec {
  pool: BufferPool,
  in_overflow: bool,
  buffer: [u8; 512],
  pos: usize,
}

impl LineCodec {
  pub fn new(capacity: usize) -> LineCodec {
    LineCodec {
      pool: BufferPool::new(512, capacity),
      in_overflow: false,
      buffer: [0; 512],
      pos: 0,
    }
  }

  pub fn add_bytes<'iter>(&'iter mut self, bytes: &'iter [u8]) -> AddBytesIter {
    debug_assert!(bytes.len() > 0);
    AddBytesIter {
      codec: self,
      buffer: bytes,
    }
  }

  pub fn lines(&mut self) -> AddBytesIter {
    AddBytesIter {
      codec: self,
      buffer: &[0u8; 0],
    }
  }

  fn shift(&mut self, n: usize) {
    debug_assert!(n > 0);
    debug_assert!(n <= self.pos);
    self.buffer.copy_within(n..self.pos, 0);
    self.pos -= n;
  }

  pub fn is_empty(&self) -> bool {
    self.pos == 0
  }
}

pub struct AddBytesIter<'a> {
  codec: &'a mut LineCodec,
  buffer: &'a [u8],
}

impl<'a> AddBytesIter<'a> {
  fn advance_buffer(&mut self) -> AdvanceBufferResult {
    while self.buffer.len() > 0 {
      if self.codec.in_overflow {
        // Look for a \r or \n, which will terminate the current line.
        for (index, ch) in self.buffer.iter().map(|ch| *ch as char).enumerate() {
          if ch == '\r' || ch == '\n' {
            self.codec.in_overflow = false;
            self.buffer = &self.buffer[(index + 1)..];
            debug_assert!(self.codec.pos == 0);
            break;
          }
        }

        if self.codec.in_overflow {
          // In overflow mode, and no matching characters were found.
          self.buffer = Default::default();
          return AdvanceBufferResult::InOverflow;
        }
      } else {
        // Add as many bytes as possible to the buffer.
        let n = std::cmp::min(self.codec.buffer.len() - self.codec.pos, self.buffer.len());
        let mut dst = &mut self.codec.buffer[self.codec.pos..];
        let src = &self.buffer[0..n];
        self.buffer = &self.buffer[n..];
        let written = dst.write(src).expect("Conditions should have been checked");
        debug_assert!(written == n);
        self.codec.pos += n;

        break;
      }
    }
    if self.buffer.len() > 0 {
      AdvanceBufferResult::BufferNotEmpty
    } else {
      AdvanceBufferResult::BufferEmpty
    }
  }
}

impl<'a> Iterator for AddBytesIter<'a> {
  type Item = Buffer;

  fn next(&mut self) -> Option<Buffer> {
    match self.advance_buffer() {
      AdvanceBufferResult::InOverflow => return None,
      _ => (),
    }

    let mut state = LineCodecState::NewlineScan;
    loop {
      match state {
        LineCodecState::SkipEmptyLine => {
          self.codec.shift(1);
          state = LineCodecState::NewlineScan;
        }
        LineCodecState::NewlineScan => {
          for (index, ch) in self.codec.buffer[0..self.codec.pos]
            .iter()
            .map(|ch| *ch as char)
            .enumerate()
          {
            if ch == '\r' || ch == '\n' {
              // Line found.
              if index == 0 {
                // No data for this line, so skip it and retry.
                state = LineCodecState::SkipEmptyLine;
              } else {
                // Line found.
                state = LineCodecState::ProduceLine(index);
              }
              break;
            }
          }

          if state != LineCodecState::NewlineScan {
            continue;
          }

          // All characters have been exhausted and no line found.
          if self.codec.pos == self.codec.buffer.len() {
            self.codec.in_overflow = true;
            state = LineCodecState::ProduceLine(self.codec.pos);
          } else {
            // No line currently available.
            return None;
          }
        }
        LineCodecState::ProduceLine(len) => {
          let line_bytes = &self.codec.buffer[0..len];

          // Attempt to convert to a string
          match std::str::from_utf8(line_bytes) {
            Ok(line_str) => {
              let mut buf = self.codec.pool.take().expect("RecvQ error?");
              buf.append(line_str).expect("Should be sized correctly");
              // Now that the bytes have been copied to 'buf', they can be reclaimed in the main
              // buffer.
              self.codec.shift(len);
              return Some(buf);
            }
            Err(_) => {
              // Ignore this line, as it contains invalid bytes.
              self.codec.shift(len);
              state = LineCodecState::NewlineScan;
              continue;
            }
          }
        }
      }
    }
  }
}

impl<'a> Drop for AddBytesIter<'a> {
  fn drop(&mut self) {
    if self.buffer.len() == 0 {
      return;
    }

    match self.advance_buffer() {
      AdvanceBufferResult::BufferEmpty | AdvanceBufferResult::InOverflow => (),
      AdvanceBufferResult::BufferNotEmpty => panic!("Recvq?"),
    }
  }
}

#[derive(PartialEq)]
enum LineCodecState {
  NewlineScan,
  SkipEmptyLine,
  ProduceLine(usize),
}

enum AdvanceBufferResult {
  BufferEmpty,
  BufferNotEmpty,
  InOverflow,
}
