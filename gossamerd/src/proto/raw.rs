use super::buffer::Buffer;

const SMALL_SIZE: usize = 6;

enum IrcArgs {
  Fixed(usize, [Option<(usize, usize)>; SMALL_SIZE]),
  VarLength(Vec<(usize, usize)>),
}

impl IrcArgs {
  fn append(&mut self, arg: (usize, usize)) {
    match self {
      IrcArgs::VarLength(args) => {
        args.push(arg);
      }
      IrcArgs::Fixed(count, args) => {
        if *count < SMALL_SIZE {
          (*args)[*count] = Some(arg);
          *count = *count + 1;
        } else {
          let rest: Vec<(usize, usize)> = (*args).iter().map(|v| v.unwrap()).collect();
          std::mem::replace(self, IrcArgs::VarLength(rest));
        }
      }
    }
  }

  fn len(&self) -> usize {
    match self {
      IrcArgs::VarLength(args) => args.len(),
      IrcArgs::Fixed(count, _) => *count,
    }
  }

  fn arg(&self, index: usize) -> (usize, usize) {
    debug_assert!(index < self.len());
    match self {
      IrcArgs::VarLength(args) => *args.get(index).unwrap(),
      IrcArgs::Fixed(_, args) => args[index].unwrap(),
    }
  }
}

pub struct RawMessage {
  buffer: Buffer,
  args: IrcArgs,
  cmd_index: usize,
}

impl RawMessage {
  pub fn len(&self) -> usize {
    self.args.len()
  }

  pub fn arg(&self, index: usize) -> &str {
    let (start, len) = match &self.args {
      IrcArgs::Fixed(arg_count, args) => {
        debug_assert!(index < *arg_count);
        args[index].unwrap()
      }
      IrcArgs::VarLength(args) => {
        debug_assert!(index < args.len());
        *args.get(index).unwrap()
      }
    };
    let buffer = self.buffer.as_str();
    if start < buffer.len() {
      // Safe because the indices were created at UTF-8 boundaries during parsing.
      unsafe { buffer.slice_unchecked(start, start + len) }
    } else {
      ""
    }
  }
  pub fn command(&self) -> &str {
    self.arg(self.cmd_index)
  }
}

impl RawMessage {
  pub fn parse(buffer: Buffer) -> RawMessage {
    let mut args = IrcArgs::Fixed(0, Default::default());
    let mut cmd_index = 0;
    let mut state = ParserState::BeforeArg;

    let bytes = buffer.as_str().as_bytes();
    for (index, ch) in bytes.iter().map(|ch| *ch as char).enumerate() {
      match state {
        ParserState::BeforeArg => {
          if ch == ':' {
            if args.len() > 0 {
              state = ParserState::InRestArg(index + 1);
              break;
            } else {
              // Leading : means nothing in the first argument.
              cmd_index = 1;
            }
          } else if ch != ' ' {
            // Argument starts here.
            state = ParserState::InArg(index);
          }
        }
        ParserState::InArg(start_idx) => {
          if ch == ' ' {
            let arg_len = index - start_idx;
            args.append((start_idx, arg_len));
            state = ParserState::BeforeArg;
          }
        }
        ParserState::InRestArg(_) => {}
      }
    }

    // Handle the final argument.
    match state {
      ParserState::InArg(start_idx) | ParserState::InRestArg(start_idx) => {
        let arg_len = bytes.len() - start_idx;
        args.append((start_idx, arg_len));
      }
      _ => {}
    }

    RawMessage {
      buffer,
      args,
      cmd_index,
    }
  }
}

enum ParserState {
  BeforeArg,
  InArg(/* start */ usize),
  InRestArg(usize),
}
