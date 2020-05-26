use super::buffer::*;
use super::raw::*;

#[test]
fn test_raw_nick() {
  let msg = parse("NICK alex");
  assert_eq!(msg.len(), 2);
  assert_eq!(msg.arg(0), "NICK");
  assert_eq!(msg.arg(1), "alex");
}

#[test]
fn test_raw_basic_command() {
  let msg = parse("NICK alex");
  assert_eq!(msg.len(), 2);
  assert_eq!(msg.command(), "NICK");
}

#[test]
fn test_raw_prefixed_command() {
  let msg = parse(":alex!alex@alex QUIT :Quit message");
  assert_eq!(msg.len(), 3);
  assert_eq!(msg.arg(0), "alex!alex@alex");
  assert_eq!(msg.command(), "QUIT");
  assert_eq!(msg.arg(2), "Quit message");
}

#[test]
fn test_raw_rest_empty() {
  let msg = parse(":alex!alex@alex QUIT :");
  assert_eq!(msg.len(), 3);
  assert_eq!(msg.arg(2), "");
}

#[test]
fn test_raw_rest() {
  let msg = parse("USER alex * * :Real name goes here");
  assert_eq!(msg.len(), 5);
  assert_eq!(msg.arg(4), "Real name goes here");
}

fn parse(msg: &'static str) -> RawMessage {
  let mut pool = BufferPool::new(msg.len(), 1);
  let mut buf = pool.take().expect("read from buffer pool");
  buf.append(msg).unwrap();

  RawMessage::parse(buf)
}
