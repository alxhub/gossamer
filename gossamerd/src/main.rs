use async_trait::async_trait;
use libgossamer::engine::{Engine, EngineHandle, Handler, Network};
use tokio::prelude::*;

// struct MyHandler {}

// #[async_trait]
// impl Handler for MyHandler {
//     async fn on_startup(&mut self, network: &mut Network) {
//         println!("startup!");
//     }
// }

#[tokio::main]
async fn main() {
    // let engine: Engine<MyHandler> = Engine::new("test".to_string(), |_| MyHandler {});
    // // let mut handle = engine.handle();
    // let res = tokio::spawn(async move {
    //     println!("Running engine");
    //     // engine.run().await;
    // });
    // // handle.shutdown().await;
    // res.await.unwrap();
    // ()
}
