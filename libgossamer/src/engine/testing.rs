use super::*;
use crate::proto::link;
use async_trait::async_trait;
use futures::channel::mpsc;
use futures::channel::oneshot::{channel, Receiver, Sender};
use futures::executor::block_on;
use futures::future::join_all;
use futures::join;
use futures::FutureExt;

pub struct TestController {
  engines: Vec<Engine<TestEvent, TestHandler>>,
  engine_ctrl: Vec<EngineHandle<TestEvent>>,
  links: Vec<TestLink>,
}

pub struct TestEngine {
  handle: EngineHandle<TestEvent>,
}

struct TestLink {
  ctrl_rx: mpsc::Receiver<LinkControl>,
  a: EngineHandle<TestEvent>,
  b: EngineHandle<TestEvent>,
}

impl TestLink {
  fn new(a: &TestEngine, b: &TestEngine, ctrl_rx: mpsc::Receiver<LinkControl>) -> TestLink {
    TestLink {
      ctrl_rx,
      a: a.handle.clone(),
      b: b.handle.clone(),
    }
  }

  async fn run(mut self) {
    println!("awaiting link startup");
    match self.ctrl_rx.next().await.unwrap() {
      LinkControl::Start(ready) => {
        println!("link starting");

        let (ab_tx, ab_rx) = mpsc::channel(32);
        let (ba_tx, ba_rx) = mpsc::channel(32);
        let (a_ready_tx, a_ready_rx) = channel();
        let (b_ready_tx, b_ready_rx) = channel();

        let (a_notify_done_tx, a_notify_done_rx) = channel();
        let (b_notify_done_tx, b_notify_done_rx) = channel();

        self
          .a
          .send_event(TestEvent::RequestLinkNotify(a_ready_tx, a_notify_done_tx))
          .await;
        self
          .b
          .send_event(TestEvent::RequestLinkNotify(b_ready_tx, b_notify_done_tx))
          .await;

        let (x, y) = join!(a_notify_done_rx, b_notify_done_rx);
        x.unwrap();
        y.unwrap();

        let ready = async {
          let (x, y) = join!(a_ready_rx, b_ready_rx);
          x.unwrap();
          y.unwrap();
          ready.send(()).unwrap();
        };

        let link_ab = link::Link::new(ab_tx, ba_rx, self.a);
        let link_ba = link::Link::new(ba_tx, ab_rx, self.b);
        join!(link_ab.run(), link_ba.run(), ready);
      }
      _ => unimplemented!(),
    }
  }
}

impl TestEngine {
  pub async fn with<
    R: std::fmt::Debug + Send + 'static,
    F: FnOnce(&mut Network) -> R + Send + 'static,
  >(
    &mut self,
    f: F,
  ) -> R {
    let (run_tx, run_rx) = channel();
    let (res_tx, res_rx) = channel();
    let wrapped_fn = move |net: &mut Network| {
      let res = f(net);
      res_tx.send(res).unwrap();
    };
    self
      .handle
      .send_event(TestEvent::Run(Box::new(wrapped_fn), run_tx))
      .await;
    run_rx.await.unwrap();
    res_rx.await.unwrap()
  }

  pub async fn subnet_add<S: Into<String>>(&mut self, name: S) -> Result<SubnetId, state::Error> {
    let (tx, rx) = channel();
    self
      .handle
      .send_event(TestEvent::SubnetAdd(name.into(), tx))
      .await;
    rx.await.unwrap()
  }

  pub async fn sync(&mut self) {
    let (tx, rx) = channel();
    self.handle.send_event(TestEvent::SyncStart(tx)).await;
    rx.await.unwrap();
  }

  pub async fn client_add<S: Into<String>>(
    &mut self,
    subnet: SubnetId,
    nick: S,
    ident: S,
    host: S,
    gecos: S,
  ) -> Result<SubnetId, state::Error> {
    let (tx, rx) = channel();
    self
      .handle
      .send_event(TestEvent::ClientAdd(
        subnet,
        nick.into(),
        ident.into(),
        host.into(),
        gecos.into(),
        tx,
      ))
      .await;
    rx.await.unwrap()
  }

  pub async fn shutdown(&mut self) {
    self.handle.shutdown().await
  }

  async fn link_notify(&mut self, sender: Sender<()>) {
    let (tx, rx) = channel();
    self
      .handle
      .send_event(TestEvent::RequestLinkNotify(sender, tx))
      .await;
    rx.await.unwrap();
  }
}

#[derive(Clone)]
pub struct LinkController {
  ctrl_tx: mpsc::Sender<LinkControl>,
}

impl LinkController {
  pub async fn start(&mut self) {
    let (tx, rx) = channel();
    self.ctrl_tx.send(LinkControl::Start(tx)).await.unwrap();
    rx.await.unwrap();
  }
}

enum LinkControl {
  Start(Sender<()>),
}

impl TestController {
  pub fn new() -> TestController {
    TestController {
      engines: Vec::new(),
      engine_ctrl: Vec::new(),
      links: Vec::new(),
    }
  }

  pub fn add_engine<S: Into<String>>(&mut self, server_name: S) -> TestEngine {
    let server_name = server_name.into();
    let engine = Engine::new(server_name, |_| TestHandler {
      link_notify: None,
      sync_notify: None,
    });
    let handle = engine.handle();
    self.engine_ctrl.push(engine.handle());
    self.engines.push(engine);
    TestEngine { handle }
  }

  pub fn add_link(&mut self, a: &TestEngine, b: &TestEngine) -> LinkController {
    let (ctrl_tx, ctrl_rx) = mpsc::channel(32);
    let link = TestLink::new(a, b, ctrl_rx);
    self.links.push(link);
    LinkController { ctrl_tx }
  }

  pub fn run<FT: Future<Output = ()>, F: FnOnce() -> FT>(mut self, f: F) {
    block_on(async move {
      let mut f_list: Vec<Pin<Box<dyn Future<Output = ()>>>> = Vec::new();

      self
        .engines
        .drain(..)
        .map(|e| Box::pin(e.run().map(|_| ())))
        .for_each(|f| f_list.push(f));
      self
        .links
        .drain(..)
        .map(|l| Box::pin(l.run().map(|_| ())))
        .for_each(|f| f_list.push(f));

      f_list.push(Box::pin(async {
        f().await;
        for mut handle in self.engine_ctrl.drain(..) {
          handle.shutdown().await;
        }
      }));
      join_all(f_list).await;
    });
  }
}

struct TestHandler {
  link_notify: Option<Sender<()>>,
  sync_notify: Option<(Sender<()>, usize)>,
}

#[async_trait]
impl Handler<TestEvent> for TestHandler {
  async fn on_startup(&mut self, network: &mut Network) {
    println!("handler startup");
  }

  async fn on_event(&mut self, network: &mut Network, event: TestEvent) {
    match event {
      TestEvent::Run(f, tx) => {
        f(network);
        tx.send(()).unwrap();
      }
      TestEvent::SubnetAdd(name, tx) => {
        tx.send(network.subnet_add(name).await).unwrap();
      }
      TestEvent::RequestLinkNotify(notify_tx, tx) => {
        if self.link_notify.is_some() {
          panic!("Requested link notification when another request was already active.");
        }
        self.link_notify = Some(notify_tx);
        tx.send(()).unwrap();
      }
      TestEvent::ClientAdd(subnet, nick, ident, host, gecos, tx) => {
        tx.send(network.client_add(subnet, nick, ident, host, gecos).await)
          .unwrap();
      }
      TestEvent::SyncStart(tx) => {
        // Count the servers.
        if self.sync_notify.is_some() {
          panic!("Requested sync notificationn when another request was already active.");
        }
        let count = network.state.server_count() - 1;
        self.sync_notify = Some((tx, count));
        network.sync().await;
      }
      _ => unimplemented!(),
    }
  }

  async fn on_link(&mut self, network: &mut Network, peer: ServerId) {
    // panic!(
    //   "{} linked to {}",
    //   network.state.server_by_id(0).name,
    //   network.state.server_by_id(peer).name
    // );
    let mut link_notify = None;
    std::mem::swap(&mut self.link_notify, &mut link_notify);
    if let Some(tx) = link_notify {
      tx.send(()).unwrap();
    }
  }

  async fn on_sync_response(&mut self, network: &mut Network, server: ServerId) {
    let mut sync_notify = None;
    std::mem::swap(&mut self.sync_notify, &mut sync_notify);
    if let Some((tx, count)) = sync_notify {
      let count = count - 1;
      if count == 0 {
        // All connected servers have responded.
        tx.send(()).unwrap();
      } else {
        // Still waiting on `count` servers.
        self.sync_notify = Some((tx, count));
      }
    }
  }
}

enum TestEvent {
  ClientAdd(
    SubnetId,
    /* nick */ String,
    /* ident */ String,
    /* host */ String,
    /* gecos */ String,
    Sender<Result<ClientId, state::Error>>,
  ),
  SubnetAdd(String, Sender<Result<SubnetId, state::Error>>),
  RequestLinkNotify(Sender<()>, Sender<()>),
  SyncStart(Sender<()>),
  Run(Box<dyn FnOnce(&mut Network) -> () + Send>, Sender<()>),
}
