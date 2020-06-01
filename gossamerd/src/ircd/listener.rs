use crate::client::ClientConnection;
use crate::ircd::IrcdEvent;
use futures::channel::mpsc;
use futures::stream::StreamExt;
use libgossamer::engine::EngineHandle;
use std::collections::HashMap;
use tokio::net::TcpListener;
use tokio::select;

pub struct Listener {
  next_id: ListenerId,
  active: HashMap<ListenerId, mpsc::Sender<Control>>,
  handle: EngineHandle<IrcdEvent>,
}

impl Listener {
  pub fn new(handle: EngineHandle<IrcdEvent>) -> Listener {
    Listener {
      next_id: 0,
      active: HashMap::new(),
      handle,
    }
  }

  pub fn listen(&mut self, addr: String) -> ListenerId {
    let id = self.next_id;
    self.next_id += 1;

    let (ctrl_tx, ctrl_rx) = mpsc::channel(3);

    self.active.insert(id, ctrl_tx);
    tokio::spawn(listen_task(id, addr, ctrl_rx, self.handle.clone()));
    id
  }
}

async fn listen_task(
  id: ListenerId,
  addr: String,
  mut ctrl_rx: mpsc::Receiver<Control>,
  handle: EngineHandle<IrcdEvent>,
) {
  let mut listener = TcpListener::bind(addr).await.unwrap();

  loop {
    select! {
      msg = ctrl_rx.next() => {
        match msg {
          Some(Control::Close) | None => return,
        }
      }
      socket = listener.accept() => {
        let (socket, _) = socket.unwrap();
        let (conn_read, conn_write) = socket.into_split();
        tokio::spawn(ClientConnection::new("hub.a".to_string(), handle.clone()).run(conn_read, conn_write));
      }
    }
  }
}

pub type ListenerId = u32;

enum Control {
  Close,
}
