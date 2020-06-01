mod listener;

use crate::client::ClientEvent;
use crate::proto::*;
use async_trait::async_trait;
use futures::channel::mpsc;
use futures::sink::SinkExt;
use libgossamer::engine::state::{self, *};
use libgossamer::engine::{EngineHandle, Handler, Network};
use listener::Listener;
use std::collections::HashMap;

mod welcome;

#[derive(Debug)]
pub enum IrcdEvent {
  AttemptRegistration {
    nick: String,
    ident: String,
    gecos: String,
    tx: mpsc::Sender<ClientEvent>,
  },
  ClientMessage(ClientId, IrcMessage),
  ClientDisconnect(ClientId),
}

pub struct Ircd {
  handle: EngineHandle<IrcdEvent>,
  local_client: HashMap<ClientId, mpsc::Sender<ClientEvent>>,
  listener: Listener,
}

impl Ircd {
  pub fn from(handle: EngineHandle<IrcdEvent>) -> Ircd {
    Ircd {
      listener: Listener::new(handle.clone()),
      local_client: HashMap::new(),
      handle,
    }
  }

  async fn on_client_message(&mut self, network: &mut Network, id: ClientId, msg: IrcMessage) {
    match msg {
      IrcMessage::Privmsg(msg) => {
        self.on_privmsg(network, id, msg).await;
      }
      _ => unimplemented!(),
    }
  }

  async fn on_client_disconnect(&mut self, network: &mut Network, id: ClientId) {
    network.client_quit(id, "Quit".to_string()).await;
  }

  async fn on_privmsg(&mut self, network: &mut Network, id: ClientId, msg: IrcPrivateMessage) {
    let target_id = network.state.client_by_nick(0, msg.target().as_ref());
    match target_id {
      Some(target_id) => {
        // Is the target local?
        if let Some(target_tx) = self.local_client.get_mut(&target_id) {
          let sender = network.state.client_by_id(id);
          let target = network.state.client_by_id(target_id);
          let msg = format!(
            ":{}!{}@{} PRIVMSG {} :{}\r\n",
            sender.nick,
            sender.ident,
            sender.host,
            target.nick,
            msg.message()
          );
          target_tx.send(ClientEvent::Send(msg)).await.unwrap();
        } else {
        }
      }
      None => panic!("unknown PRIVMSG target"),
    }
  }
}

#[async_trait]
impl Handler<IrcdEvent> for Ircd {
  async fn on_startup(&mut self, network: &mut Network) {
    network.subnet_add("dev").await.unwrap();
    self.listener.listen("127.0.0.1:6667".to_string());
  }

  async fn on_event(&mut self, network: &mut Network, event: IrcdEvent) {
    match event {
      IrcdEvent::AttemptRegistration {
        nick,
        ident,
        gecos,
        mut tx,
      } => {
        match network
          .client_add(0, nick, ident, "127.0.0.1".to_string(), gecos)
          .await
        {
          Ok(id) => {
            self.local_client.insert(id, tx.clone());
            tx.send(ClientEvent::RegistrationSuccess(id)).await.unwrap();
            self.send_welcome(network, id, tx).await.unwrap();
          }
          Err(state::Error::NickAlreadyInUse(_)) => {
            tx.send(ClientEvent::RegistrationNickTaken).await.unwrap();
          }
          _ => unreachable!(),
        };
      }
      IrcdEvent::ClientMessage(id, msg) => self.on_client_message(network, id, msg).await,
      IrcdEvent::ClientDisconnect(id) => self.on_client_disconnect(network, id).await,
    }
  }

  async fn on_link(&mut self, network: &mut Network, peer: ServerId) {
    unimplemented!()
  }

  async fn on_sync_response(&mut self, network: &mut Network, server: ServerId) {
    unreachable!("test code only")
  }
  async fn on_netsplit(&mut self, network: &mut Network, peer: ServerId, from: ServerId) {
    unimplemented!()
  }
}
