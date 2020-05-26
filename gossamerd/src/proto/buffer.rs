use std::fmt::Write;
use std::sync::mpsc;

pub struct Buffer {
  contents: String,
  return_tx: mpsc::Sender<String>,
}

impl Buffer {
  pub fn new(size: usize, return_tx: mpsc::Sender<String>) -> Buffer {
    Buffer {
      contents: String::with_capacity(size),
      return_tx,
    }
  }

  pub fn wrap(contents: String, return_tx: mpsc::Sender<String>) -> Buffer {
    Buffer {
      contents,
      return_tx,
    }
  }

  pub fn as_str(&self) -> &str {
    self.contents.as_ref()
  }

  pub fn as_mut_str(&mut self) -> &mut str {
    self.contents.as_mut()
  }

  pub fn append(&mut self, value: &str) -> Result<(), ()> {
    if value.len() <= self.contents.capacity() - self.contents.len() {
      self.contents += value;
      Ok(())
    } else {
      Err(())
    }
  }
}

impl Drop for Buffer {
  fn drop(&mut self) {
    let contents = std::mem::replace(&mut self.contents, String::default());
    let _ = self.return_tx.send(contents);
  }
}

pub struct BufferPool {
  buffer_size: usize,
  buffers: Vec<Buffer>,
  loaned: usize,
  return_rx: mpsc::Receiver<String>,
  return_tx: mpsc::Sender<String>,
}

impl BufferPool {
  pub fn new(size: usize, count: usize) -> BufferPool {
    let buffers = Vec::with_capacity(count);
    let (return_tx, return_rx) = mpsc::channel();
    BufferPool {
      buffer_size: size,
      buffers,
      loaned: 0,
      return_rx,
      return_tx,
    }
  }

  pub fn take(&mut self) -> Result<Buffer, BufferError> {
    self.maybe_recover_buffers();

    // If no buffers are available but it's possible to allocate a new one, do that.
    if self.buffers.is_empty() && self.loaned < self.buffers.capacity() {
      self
        .buffers
        .push(Buffer::new(self.buffer_size, self.return_tx.clone()))
    }

    let res = self.buffers.pop().ok_or(BufferError::Empty);
    if res.is_ok() {
      self.loaned += 1;
    }
    res
  }

  pub fn size_buffers(&self) -> usize {
    self.loaned + self.buffers.len()
  }

  fn maybe_recover_buffers(&mut self) {
    for mut contents in self.return_rx.try_iter() {
      contents.truncate(0);
      self
        .buffers
        .push(Buffer::wrap(contents, self.return_tx.clone()));
      debug_assert!(self.loaned > 0);
      self.loaned -= 1;
    }
  }
}

#[derive(Debug, Copy, Clone)]
pub enum BufferError {
  Empty,
}
