mod client;
mod ircd;
mod proto;

use ircd::{Ircd, IrcdEvent};
use libgossamer::engine::Engine;

#[tokio::main]
async fn main() {
    let engine: Engine<IrcdEvent, Ircd> =
        Engine::new("hub.a".to_string(), |handle| Ircd::from(handle));
    engine.run().await;
    ()
}
