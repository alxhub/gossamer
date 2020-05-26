use super::raw::*;

pub enum IrcMessage {
  NickRequest(IrcNickRequestMessage),
}

pub enum IrcMessageError {
  WrongArgCount(RawMessage),
}

impl IrcMessage {
  fn from(msg: RawMessage) -> Result<IrcMessage, IrcMessageError> {
    let cmd = msg.command();
    if cmd.eq_ignore_ascii_case("NICK") {
      if msg.len() != 1 {
        return Err(IrcMessageError::WrongArgCount(msg));
      } else {
        return Ok(IrcNickRequestMessage::from(msg));
      }
    }
    unimplemented!()
  }
}

pub struct IrcNickRequestMessage {
  msg: RawMessage,
}

impl IrcNickRequestMessage {
  fn from(msg: RawMessage) -> IrcMessage {
    IrcMessage::NickRequest(IrcNickRequestMessage { msg })
  }

  pub fn nick(&self) -> &str {
    self.msg.arg(1)
  }
}
