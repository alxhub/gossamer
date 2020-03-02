use super::state::*;
use super::testing::*;
use super::*;
use async_trait::async_trait;
use futures::executor::block_on;
use futures::join;

// #[test]
// fn test_one() {
//   let mut ctrl = TestController::new();
//   let mut h = ctrl.add_engine("hub.test");
//   ctrl.run(|| {
//     async {
//       println!("startup controller");
//       let subnet = h.subnet_add("dev").await.unwrap();
//       let client = h
//         .client_add(subnet, "test", "t", "test.dev", "Test")
//         .await
//         .unwrap();
//       h.with(move |net| {
//         println!("assertions");
//         assert_eq!(net.state.client_by_nick(subnet, "test"), Some(client));
//       })
//       .await;
//       h.shutdown().await;
//     }
//   });
// }

#[test]
fn test_link() {
  let mut ctrl = TestController::new();
  let mut a = ctrl.add_engine("hub.a");
  let mut b = ctrl.add_engine("hub.b");
  let mut link_ab = ctrl.add_link(&a, &b);
  ctrl.run(|| async {
    a.subnet_add("test").await.unwrap();
    link_ab.start().await;
    b.with(|net| {
      let linked_b = net.state.server_by_id(1);
      assert_eq!(linked_b.name, "hub.a");
      let sn_b = net.state.subnet_by_name("test");
      assert!(sn_b.is_some(), "Subnet should exist on hub.b");
    })
    .await;
  });
}

#[test]
fn test_client_link() {
  let mut ctrl = TestController::new();
  let mut a = ctrl.add_engine("hub.a");
  let mut b = ctrl.add_engine("hub.b");
  let mut link_ab = ctrl.add_link(&a, &b);
  ctrl.run(|| async {
    let sn = a.subnet_add("dev").await.unwrap();
    a.client_add(sn, "test", "test", "test.client", "Test Client")
      .await
      .unwrap();
    link_ab.start().await;
    b.with(move |net| {
      let client = net.state.client_by_nick(sn, "test");
      assert!(client.is_some(), "Client should exist on hub.b");
    })
    .await;
    b.client_add(sn, "test2", "test2", "test.client", "Test Client")
      .await
      .unwrap();
    a.with(move |net| {
      let client = net.state.client_by_nick(sn, "test2");
      assert!(client.is_some(), "Client should exist on hub.a");
    })
    .await;
  });
}

#[test]
fn test_netsplit() {
  let mut ctrl = TestController::new();
  let mut a = ctrl.add_engine("hub.a");
  let mut b = ctrl.add_engine("hub.b");
  let mut link_ab = ctrl.add_link(&a, &b);
  ctrl.run(|| async {
    let sn = a.subnet_add("dev").await.unwrap();
    a.client_add(sn, "test", "test", "test.client", "Test Client")
      .await
      .unwrap();
    link_ab.start().await;

    b.with(move |net| {
      let client = net.state.client_by_nick(sn, "test");
      assert!(client.is_some(), "Client should exist on hub.b");
    })
    .await;

    link_ab.stop().await;

    b.with(move |net| {
      let client = net.state.client_by_nick(sn, "test");
      assert!(client.is_none(), "Client should not exist on hub.b");
    })
    .await;
  });
}
