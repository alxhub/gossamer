use async_trait::async_trait;
use libgossamer::engine::state::ServerId;
use libgossamer::engine::{Engine, EngineHandle, Handler, Network};
use tokio::prelude::*;

struct IrcdEvent {}

struct Ircd {}

#[async_trait]
impl Handler<IrcdEvent> for Ircd {
    async fn on_startup(&mut self, network: &mut Network) {
        println!("startup!");
    }

    async fn on_event(&mut self, network: &mut Network, event: IrcdEvent) {
        unimplemented!()
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

#[tokio::main]
async fn main() {
    let engine: Engine<IrcdEvent, Ircd> = Engine::new("hub.a".to_string(), |_| Ircd {});
    engine.run().await;
    ()
}
