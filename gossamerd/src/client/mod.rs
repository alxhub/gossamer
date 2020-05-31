use crate::proto::{Buffer, IrcMessage, LineCodec};
use futures::channel::mpsc;
use futures::pin_mut;
use futures::sink::{Sink, SinkExt};
use futures::stream::{Stream, StreamExt};
use libgossamer::engine::state::ClientId;
use libgossamer::engine::EngineHandle;
use std::io::Write;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::select;

use crate::ircd::IrcdEvent;

#[derive(Debug)]
pub enum ClientEvent {
  RegistrationSuccess(ClientId),
  RegistrationNickTaken,
  Send(String),
}

pub trait ClientDriver {
  fn try_register(
    &mut self,
    nick: String,
    ident: String,
    gecos: String,
    tx: mpsc::Sender<ClientEvent>,
  );

  fn on_message(&mut self, id: ClientId);
}

pub struct ClientConnection {
  state: ClientState,

  ircd_tx: EngineHandle<IrcdEvent>,

  event_rx: Option<mpsc::Receiver<ClientEvent>>,
  event_tx: mpsc::Sender<ClientEvent>,

  sendq_buffer: Box<[u8]>,
  sendq_pos: usize,
}

enum ClientState {
  Pending(PendingClientState),
  RegistrationInProgress(PendingClientState),
  Active(ClientId),
}

#[derive(Default)]
struct PendingClientState {
  nick: Option<String>,
  ident: Option<String>,
  gecos: Option<String>,
}

impl ClientConnection {
  pub fn new(server_name: &'_ str, ircd_tx: EngineHandle<IrcdEvent>) -> ClientConnection {
    let mut sendq_buffer: Box<[u8]> = Box::new([0u8; 4096]);
    let notice = format!(
      ":{} NOTICE * :*** Looking up your hostname...\r\n:{} NOTICE * :*** Found your hostname\r\n",
      server_name, server_name
    );
    let sendq_pos = sendq_buffer.as_mut().write(notice.as_bytes()).unwrap();

    let (event_tx, event_rx) = mpsc::channel(5);

    ClientConnection {
      state: ClientState::Pending(Default::default()),
      ircd_tx,
      event_rx: Some(event_rx),
      event_tx,
      sendq_buffer,
      sendq_pos,
    }
  }

  pub async fn run<R: AsyncRead, W: AsyncWrite>(mut self, conn_read: R, conn_write: W) {
    // First, deconstruct self.
    let event_rx = self.event_rx.take().unwrap();
    pin_mut!(conn_read);
    pin_mut!(conn_write);
    pin_mut!(event_rx);

    let mut read_buf = [0u8; 512];
    let mut codec = LineCodec::new(8);

    loop {
      select! {
        res = conn_write.write(&self.sendq_buffer[0..self.sendq_pos]), if self.sendq_pos > 0 => {
          let n = res.unwrap();
          self.sendq_buffer.copy_within(n..self.sendq_pos, 0);
          self.sendq_pos -= n;
        }
        res = conn_read.read(&mut read_buf), if !self.state.is_buffering() => {
          let n = res.unwrap();
          self.process_bytes(&read_buf[0..n], &mut codec).await;
        }
        ev = event_rx.next() => {
          let ev = ev.unwrap();
          match ev {
            ClientEvent::RegistrationSuccess(id) => {
              self.state = ClientState::Active(id);
            }
            ClientEvent::RegistrationNickTaken => {
              panic!("nick taken");
            }
            ClientEvent::Send(msg) => {
              if self.sendq_buffer.len() - self.sendq_pos >= msg.len() {
                self.sendq_pos += self.sendq_buffer.as_mut().write(msg.as_ref()).unwrap();
              } else {
                panic!("sendq exceeded");
              }
            },
          }
        }
      }
    }
  }

  async fn process_bytes(&mut self, bytes: &[u8], codec: &mut LineCodec) {
    let lines = codec.add_bytes(bytes);
    if self.state.is_buffering() {
      return;
    }

    for line in lines {
      let msg = IrcMessage::from(line).unwrap();
      self.process_message(msg).await;

      if self.state.is_buffering() {
        return;
      }
    }
  }

  async fn process_message(&mut self, msg: IrcMessage) {
    match &mut self.state {
      ClientState::Pending(ref mut reg) => {
        match msg {
          IrcMessage::NickRequest(msg) => {
            reg.nick = Some(msg.nick().to_string());
          }
          IrcMessage::User(msg) => {
            reg.ident = Some(msg.ident().to_string());
            reg.gecos = Some(msg.gecos().to_string());
          }
          _ => unimplemented!(),
        };
        // Register if possible.
        if reg.nick.is_some() && reg.ident.is_some() && reg.gecos.is_some() {
          self
            .ircd_tx
            .send_event(IrcdEvent::AttemptRegistration {
              nick: reg.nick.clone().unwrap(),
              ident: reg.ident.clone().unwrap(),
              gecos: reg.gecos.clone().unwrap(),
              tx: self.event_tx.clone(),
            })
            .await;
        }
      }
      ClientState::Active(id) => {
        let id = *id;
        self
          .ircd_tx
          .send_event(IrcdEvent::ClientMessage(id, msg))
          .await;
      }
      _ => unreachable!(),
    }
  }
}

impl ClientState {
  fn is_buffering(&self) -> bool {
    match self {
      ClientState::RegistrationInProgress(_) => true,
      _ => false,
    }
  }
}
