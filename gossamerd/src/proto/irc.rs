use super::buffer::Buffer;
use super::raw::*;

#[derive(Debug)]
pub enum IrcMessage {
  NickRequest(IrcNickRequestMessage),
  User(IrcUserMessage),
  Privmsg(IrcPrivateMessage),
}

#[derive(Debug)]
pub enum IrcMessageError {
  WrongArgCount(RawMessage),
}

impl IrcMessage {
  pub fn from(buf: Buffer) -> Result<IrcMessage, IrcMessageError> {
    let msg = RawMessage::parse(buf);
    let cmd = msg.command();
    if cmd.eq_ignore_ascii_case("NICK") {
      if msg.len() != 2 {
        return Err(IrcMessageError::WrongArgCount(msg));
      } else {
        return Ok(IrcNickRequestMessage::wrapped(msg));
      }
    } else if cmd.eq_ignore_ascii_case("USER") {
      if msg.len() != 5 {
        return Err(IrcMessageError::WrongArgCount(msg));
      } else {
        return Ok(IrcUserMessage::wrapped(msg));
      }
    } else if cmd.eq_ignore_ascii_case("PRIVMSG") {
      if msg.len() != 3 {
        return Err(IrcMessageError::WrongArgCount(msg));
      } else {
        return Ok(IrcPrivateMessage::wrapped(msg));
      }
    }
    unimplemented!()
  }
}

#[derive(Debug)]
pub struct IrcNickRequestMessage {
  msg: RawMessage,
}

impl IrcNickRequestMessage {
  fn wrapped(msg: RawMessage) -> IrcMessage {
    IrcMessage::NickRequest(IrcNickRequestMessage { msg })
  }

  pub fn nick(&self) -> &str {
    self.msg.arg(1)
  }
}

#[derive(Debug)]
pub struct IrcUserMessage {
  msg: RawMessage,
}

impl IrcUserMessage {
  fn wrapped(msg: RawMessage) -> IrcMessage {
    IrcMessage::User(IrcUserMessage { msg })
  }

  pub fn ident(&self) -> &str {
    self.msg.arg(1)
  }

  pub fn gecos(&self) -> &str {
    self.msg.arg(4)
  }
}

#[derive(Debug)]
pub struct IrcPrivateMessage {
  msg: RawMessage,
}

impl IrcPrivateMessage {
  fn wrapped(msg: RawMessage) -> IrcMessage {
    IrcMessage::Privmsg(IrcPrivateMessage { msg })
  }

  pub fn target(&self) -> &str {
    self.msg.arg(1)
  }

  pub fn message(&self) -> &str {
    self.msg.arg(2)
  }
}
