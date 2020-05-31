pub mod state;

use crate::proto::link;
use async_trait::async_trait;
use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::channel::oneshot;
use futures::{pin_mut, Future, FutureExt, SinkExt, StreamExt};
use queues::*;
use state::*;
use std::collections::{HashMap, HashSet};
use std::pin::Pin;

/// `Network` is the heart and soul of a libgossamer network node, including sending messages to
/// other linked servers.
///
/// It provides the (async) API by which consumers of libgossamer can effect network mutations.
pub struct Network {
  pub state: State,
  links: HashMap<ServerId, Sender<link::Control>>,
}

impl Network {
  pub fn new(server_name: String) -> Network {
    Network {
      state: State::new(server_name),
      links: HashMap::new(),
    }
  }

  pub async fn client_add<T: Into<String>>(
    &mut self,
    subnet: SubnetId,
    nick: T,
    ident: T,
    host: T,
    gecos: T,
  ) -> Result<ClientId, state::Error> {
    // Add a client to the network.
    let res = self.state.client_add(
      nick.into(),
      ident.into(),
      host.into(),
      gecos.into(),
      /* server */ 0,
      subnet,
    );
    if let Ok(id) = res {
      let client = self.state.client_by_id(id);
      let server = self.state.server_by_id(client.server);
      let subnet = self.state.subnet_by_id(client.subnet);

      let msg = link::Message::Client(link::Client {
        nick: client.nick.clone(),
        ident: client.ident.clone(),
        host: client.host.clone(),
        gecos: client.gecos.clone(),
        server: server.name.clone(),
        subnet: subnet.name.clone(),
      });
      self.broadcast(msg, None).await;

      Ok(id)
    } else {
      res
    }
  }

  pub async fn subnet_add<T: Into<String>>(&mut self, name: T) -> Result<SubnetId, state::Error> {
    let res = self.state.subnet_add(name.into());
    if let Ok(id) = res {
      let name = self.state.subnet_by_id(id).name.clone();
      self
        .broadcast(link::Message::Subnet(link::Subnet { name }), None)
        .await;
    }
    res
  }

  pub async fn sync(&mut self) {
    self
      .broadcast(
        link::Message::SyncRequest(link::SyncRequest {
          from: self.state.server_by_id(0).name.clone(),
        }),
        None,
      )
      .await;
  }

  async fn subnet_from_net(&mut self, msg: link::Subnet, from: ServerId) {
    let res = self.state.subnet_add(msg.name.clone());
    match res {
      Ok(_) => {
        self
          .broadcast(link::Message::Subnet(msg.clone()), Some(from))
          .await;
      }
      Err(_) => panic!("failed to add subnet from network: {}", msg.name),
    }
  }

  async fn client_from_net(&mut self, msg: link::Client, from: ServerId) {
    let subnet = self
      .state
      .subnet_by_name(&msg.subnet)
      .expect("TODO: netsplit");
    let server = self
      .state
      .server_by_name(&msg.server)
      .expect("TODO: netplit");
    let msg_copy = msg.clone();
    self
      .state
      .client_add(msg.nick, msg.ident, msg.host, msg.gecos, server, subnet)
      .expect("TODO: netsplit");
    self
      .broadcast(link::Message::Client(msg_copy), Some(from))
      .await;
  }

  async fn server_from_net(&mut self, msg: link::Server, from: ServerId) {
    let msg_copy = msg.clone();
    let hub = self
      .state
      .server_by_name(&msg.hub_lname)
      .expect("TODO: netsplit, unable to find hub of incoming server");
    let res = self.state.server_add(msg.name, hub);
    if let Ok(_) = res {
      self
        .broadcast(link::Message::Server(msg_copy), Some(from))
        .await;
    } else {
      panic!(
        "Failed to add server {} (hub {})",
        msg_copy.name, msg_copy.hub_lname
      );
    }
  }

  async fn netsplit_from_net(&mut self, msg: link::Netsplit, from: ServerId) {
    let msg_copy = msg.clone();

    let peer = self
      .state
      .server_by_name(&msg.server)
      .expect("Unknown netsplit server");

    let split_clients = self.state.calculate_netsplit_affected_clients(peer);
    for id in split_clients {
      self.state.client_remove(id);
    }

    self.state.server_remove(peer);

    self
      .broadcast(link::Message::Netsplit(msg_copy), Some(from))
      .await;
  }

  async fn sync_request(&mut self, msg: link::SyncRequest, from: ServerId) {
    self
      .broadcast(link::Message::SyncRequest(msg.clone()), Some(from))
      .await;
    self
      .route_to(
        from,
        link::Message::SyncResponse(link::SyncResponse {
          from: self.state.server_by_id(0).lname.clone(),
          to: msg.from,
        }),
      )
      .await;
  }

  async fn link_new(
    &mut self,
    name: String,
    link_tx: Sender<link::Control>,
  ) -> Result<ServerId, state::Error> {
    let id = self.state.server_add(name.clone(), 0)?;

    self
      .broadcast(
        link::Message::Server(link::Server {
          name,
          hub_lname: self.state.server_by_id(0).lname.clone(),
        }),
        Some(id),
      )
      .await;

    self.links.insert(id, link_tx);
    self.link_burst(id).await;

    Ok(id)
  }

  async fn link_burst(&mut self, peer: ServerId) {
    let link_tx = self.links.get_mut(&peer).unwrap();
    // Burst all the servers.
    link_burst_servers(&self.state, peer, link_tx).await;
    link_burst_subnets(&self.state, link_tx).await;
    link_burst_clients(&self.state, link_tx).await;
  }

  async fn link_split(&mut self, peer: ServerId, split_clients: HashSet<ClientId>) {
    let server = self.state.server_by_id(peer);
    let server_lname = server.lname.clone();
    self.links.remove(&peer);
    self
      .broadcast(
        link::Message::Netsplit(link::Netsplit {
          server: server_lname,
        }),
        Some(peer),
      )
      .await;
    // Resolve the impact on the network.
    for id in split_clients {
      self.state.client_remove(id);
    }

    self.state.server_remove(peer);
  }

  async fn broadcast(&mut self, msg: link::Message, skip: Option<ServerId>) {
    for (id, link) in self.links.iter_mut() {
      if Some(*id) == skip {
        continue;
      }

      link.send(link::Control::Send(msg.clone())).await.unwrap();
    }
  }

  async fn route_to(&mut self, target: ServerId, msg: link::Message) {
    let target = self.state.server_by_id(target);
    let route = target.link.unwrap().route;
    self
      .links
      .get_mut(&route)
      .unwrap()
      .send(link::Control::Send(msg))
      .await
      .unwrap();
  }
}

async fn link_burst_servers(state: &State, peer: ServerId, link_tx: &mut Sender<link::Control>) {
  let mut queue: Queue<ServerId> = Queue::new();
  queue.add(0).unwrap();

  while let Ok(id) = queue.remove() {
    let server = state.server_by_id(id);

    // id 0 is already taken care of in Hello. peer is the remote server being bursted, so skip that
    // one too.
    if id != 0 && id != peer {
      let name = server.name.clone();
      let hub_lname = state.server_by_id(server.link.unwrap().hub).lname.clone();
      link_tx
        .send(link::Control::Send(link::Message::Server(link::Server {
          name,
          hub_lname,
        })))
        .await
        .unwrap();
    }

    for downlink in server.downlinks.iter() {
      queue.add(*downlink).unwrap();
    }
  }
}

async fn link_burst_subnets(state: &State, link_tx: &mut Sender<link::Control>) {
  for id in state.subnet_iter() {
    let subnet = state.subnet_by_id(*id);

    link_tx
      .send(link::Control::Send(link::Message::Subnet(link::Subnet {
        name: subnet.name.clone(),
      })))
      .await
      .unwrap();
  }
}

async fn link_burst_clients(state: &State, link_tx: &mut Sender<link::Control>) {
  for id in state.client_iter() {
    let client = state.client_by_id(*id);
    let subnet = state.subnet_by_id(client.subnet);
    let server = state.server_by_id(client.server);

    link_tx
      .send(link::Control::Send(link::Message::Client(link::Client {
        nick: client.nick.clone(),
        ident: client.ident.clone(),
        host: client.host.clone(),
        gecos: client.gecos.clone(),
        subnet: subnet.name.clone(),
        server: server.name.clone(),
      })))
      .await
      .unwrap();
  }
}

pub struct EngineHandle<E: Send> {
  ctrl_tx: Sender<EngineControlMsg<E>>,
}

impl<E: Send> EngineHandle<E> {
  pub async fn shutdown(&mut self) {
    self
      .ctrl_tx
      .send(EngineControlMsg::Shutdown())
      .await
      .unwrap();
  }

  pub async fn send_event(&mut self, event: E) {
    self
      .ctrl_tx
      .send(EngineControlMsg::Event(event))
      .await
      .unwrap();
  }

  pub fn send_event_sync(&mut self, event: E) {
    self
      .ctrl_tx
      .try_send(EngineControlMsg::Event(event))
      .unwrap();
  }
}

impl<E: Send> Clone for EngineHandle<E> {
  fn clone(&self) -> EngineHandle<E> {
    EngineHandle {
      ctrl_tx: self.ctrl_tx.clone(),
    }
  }
}

#[async_trait]
impl<E: Send> link::Controller for EngineHandle<E> {
  async fn link_provisional(&mut self, ctrl_tx: Sender<link::Control>) {
    self
      .ctrl_tx
      .send(EngineControlMsg::LinkSendHello(ctrl_tx))
      .await
      .unwrap();
  }

  async fn link_new(
    &mut self,
    link_tx: Sender<link::Control>,
    name: String,
  ) -> Result<ServerId, state::Error> {
    let (res_tx, res_rx) = oneshot::channel();
    self
      .ctrl_tx
      .send(EngineControlMsg::LinkAccept(name, link_tx, res_tx))
      .await
      .unwrap();
    res_rx.await.unwrap()
  }

  async fn link_msg(&mut self, id: ServerId, msg: link::Message) {
    self
      .ctrl_tx
      .send(EngineControlMsg::LinkMessage(id, msg))
      .await
      .unwrap();
  }

  async fn link_closed(&mut self, id: ServerId) {
    // Note: there's a catch-22 here, where telling the engine to Shutdown() causes it to close its
    // links, which will in turn attempt to send LinkClosed messages to the engine via the
    // controller reference they have. However, after Shutdown() the engine is no longer responsive
    // to control messages. try_send() here is ensuring that in case the control channel is closed,
    // no crash happens.
    // TODO: think of a better way to handle this case.
    self.ctrl_tx.try_send(EngineControlMsg::LinkClosed(id));
  }
}

/// `Engine` is the driving half of the Network+Engine combo. It listens to messages from other
/// servers and applies their effects to the `Network`.
pub struct Engine<E: Send, T: Handler<E>> {
  network: Network,
  handler: T,
  ctrl_rx: Receiver<EngineControlMsg<E>>,
  _handle: EngineHandle<E>,
}

impl<E: Send, T: Handler<E>> Engine<E, T> {
  pub fn new<F: FnOnce(EngineHandle<E>) -> T>(server_name: String, factory: F) -> Engine<E, T> {
    let (ctrl_tx, ctrl_rx) = channel(32);
    let handle = EngineHandle { ctrl_tx };
    let handler = factory(handle.clone());
    Engine {
      network: Network::new(server_name),
      handler,
      ctrl_rx,
      _handle: handle,
    }
  }

  pub fn handle(&self) -> EngineHandle<E> {
    self._handle.clone()
  }

  async fn on_link_split(&mut self, id: ServerId) {
    let split_clients = self.network.state.calculate_netsplit_affected_clients(id);
    self.handler.on_netsplit(&mut self.network, id, 0);
    self.network.link_split(id, split_clients).await;
  }

  async fn on_link_message(&mut self, id: ServerId, msg: link::Message) {
    match msg {
      link::Message::Subnet(subnet) => self.network.subnet_from_net(subnet, id).await,
      link::Message::Client(client) => self.network.client_from_net(client, id).await,
      link::Message::Netsplit(netsplit) => {
        let split = self
          .network
          .state
          .server_by_name(&netsplit.server)
          .expect("Cannot find netsplit server?");
        let split_from = self
          .network
          .state
          .server_by_id(split)
          .link
          .map(|link| link.hub)
          .expect("Cannot find hub of split server");
        // TODO: think about the timing here. Maybe this should happen after netsplit_from_net, so
        // the handler observes the post-split network.
        self
          .handler
          .on_netsplit(&mut self.network, split, split_from)
          .await;
        self.network.netsplit_from_net(netsplit, id).await
      }
      link::Message::Server(server) => self.network.server_from_net(server, id).await,
      link::Message::SyncRequest(req) => self.network.sync_request(req, id).await,
      link::Message::SyncResponse(resp) => {
        // Check if the response is for this server.
        let target = self.network.state.server_by_name(&resp.to).expect(&format!(
          "TODO: netsplit (syncresponse target not found): {} @ {}",
          resp.to,
          self.network.state.server_by_id(0).name
        ));
        if target == 0 {
          let from = self
            .network
            .state
            .server_by_name(&resp.from)
            .expect("TODO: netsplit");
          self.handler.on_sync_response(&mut self.network, from).await;
        } else {
          // Forward to the target.
          self
            .network
            .route_to(target, link::Message::SyncResponse(resp))
            .await;
        }
      }
      _ => unimplemented!(),
    }
  }

  pub async fn run(mut self) -> Network {
    self.handler.on_startup(&mut self.network).await;

    while let Some(msg) = self.ctrl_rx.next().await {
      match msg {
        EngineControlMsg::LinkSendHello(mut tx) => {
          let name = self.network.state.server_by_id(0).name.clone();
          tx.send(link::Control::Send(link::Message::Hello(link::Hello {
            name,
          })))
          .await
          .unwrap();
        }
        EngineControlMsg::LinkAccept(name, link_tx, res_tx) => {
          let res = self.network.link_new(name, link_tx).await;
          if let Ok(id) = res {
            self.handler.on_link(&mut self.network, id).await;
          }
          res_tx.send(res).unwrap();
        }
        EngineControlMsg::LinkClosed(id) => {
          self.on_link_split(id).await;
        }
        EngineControlMsg::LinkMessage(id, msg) => self.on_link_message(id, msg).await,
        EngineControlMsg::Event(e) => {
          self.handler.on_event(&mut self.network, e).await;
        }
        EngineControlMsg::Shutdown() => {
          // panic!("Shutting down {}", self.network.state.server_by_id(0).name);
          for (_, v) in self.network.links.iter_mut() {
            v.send(link::Control::Close).await.unwrap();
          }
          return self.network;
        }
      }
    }

    panic!("Control channel for Engine ended abruptly.")
  }
}

/// The API implemented by clients of libgossamer, to respond to events on the network.
#[async_trait]
pub trait Handler<E> {
  async fn on_startup(&mut self, network: &mut Network);
  async fn on_event(&mut self, network: &mut Network, event: E);
  async fn on_link(&mut self, network: &mut Network, peer: ServerId);
  async fn on_sync_response(&mut self, network: &mut Network, server: ServerId);
  async fn on_netsplit(&mut self, network: &mut Network, split: ServerId, from: ServerId);
}

enum EngineControlMsg<E> {
  LinkSendHello(Sender<link::Control>),
  LinkAccept(
    String,
    Sender<link::Control>,
    oneshot::Sender<Result<ServerId, state::Error>>,
  ),
  LinkMessage(ServerId, link::Message),
  LinkClosed(ServerId),
  Event(E),
  Shutdown(),
}

#[cfg(test)]
mod testing;

#[cfg(test)]
mod test_basic;
