use crate::engine::state::{self, ServerId};
use async_trait::async_trait;
use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::select;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Hello {
  pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Subnet {
  pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Server {
  pub name: String,
  pub hub_lname: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Client {
  pub server: String,
  pub subnet: String,
  pub nick: String,
  pub ident: String,
  pub host: String,
  pub gecos: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SyncRequest {
  pub from: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SyncResponse {
  pub from: String,
  pub to: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum Message {
  Hello(Hello),
  Subnet(Subnet),
  Client(Client),
  Server(Server),
  SyncRequest(SyncRequest),
  SyncResponse(SyncResponse),
}

pub enum Control {
  Send(Message),
  Close,
}

enum LinkState {
  Fresh,
  Established(ServerId),
}

pub struct Link {
  state: LinkState,
  ctrl: Box<dyn Controller>,
  remote_rx: Receiver<Message>,
  remote_tx: Sender<Message>,
  ctrl_rx: Receiver<Control>,
  ctrl_tx: Sender<Control>,
}

impl Link {
  pub fn new<C: Controller + 'static>(
    remote_tx: Sender<Message>,
    remote_rx: Receiver<Message>,
    ctrl: C,
  ) -> Link {
    let (ctrl_tx, ctrl_rx) = channel(32);
    Link {
      state: LinkState::Fresh,
      ctrl: Box::new(ctrl),
      remote_rx,
      remote_tx,
      ctrl_tx,
      ctrl_rx,
    }
  }

  pub async fn process(&mut self, msg: Message) {
    match self.state {
      LinkState::Fresh => self.process_fresh(msg).await,
      LinkState::Established(id) => self.ctrl.link_msg(id, msg).await,
    }
  }

  async fn process_fresh(&mut self, msg: Message) {
    match msg {
      Message::Hello(msg) => {
        match self.ctrl.link_new(self.ctrl_tx.clone(), msg.name).await {
          Ok(id) => {
            self.state = LinkState::Established(id);
          }
          Err(_) => {
            panic!("link failed due to error");
          }
        };
      }
      _ => unreachable!(),
    }
  }

  async fn process_control(&mut self, msg: Control) {
    match msg {
      Control::Send(msg) => {
        println!("link sending: {:?}", msg);
        self.remote_tx.send(msg).await.unwrap();
      }
      _ => unimplemented!(),
    }
  }

  pub async fn run(mut self) {
    self.ctrl.link_provisional(self.ctrl_tx.clone()).await;
    loop {
      select! {
        msg = self.remote_rx.next() => {
          if let Some(msg) = msg {
            println!("got: {:?}", msg);
            self.process(msg).await;
          } else {
            panic!("remote connection has terminated!");
          }
        },
        msg = self.ctrl_rx.next() => {
          if let Some(msg) = msg {
            match msg {
              Control::Close => return,
              _ => self.process_control(msg).await,
            }
          } else {
            panic!("control connection terminated prematurely!");
          }
        },
      }
    }
  }
}

#[async_trait]
pub trait Controller {
  async fn link_provisional(&mut self, ctrl_tx: Sender<Control>);

  async fn link_new(
    &mut self,
    ctrl_tx: Sender<Control>,
    name: String,
  ) -> Result<ServerId, state::Error>;

  async fn link_msg(&mut self, id: ServerId, msg: Message);
}
