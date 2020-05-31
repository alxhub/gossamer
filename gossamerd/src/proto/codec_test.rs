use super::codec::*;

#[test]
fn test_codec_single_line() {
  let mut codec = LineCodec::new(10);
  let mut res = codec.add_bytes("Hello world\n".as_bytes());
  assert_eq!(res.next().unwrap().as_str(), "Hello world");
  assert_eq!(res.next().is_none(), true);
  drop(res);
  assert_eq!(codec.is_empty(), true);
}

#[test]
fn test_codec_multi_line() {
  let mut codec = LineCodec::new(10);
  let mut res = codec.add_bytes("Hello\rDear\r\nWorld\n\n\n".as_bytes());
  assert_eq!(res.next().unwrap().as_str(), "Hello");
  assert_eq!(res.next().unwrap().as_str(), "Dear");
  assert_eq!(res.next().unwrap().as_str(), "World");
  assert_eq!(res.next().is_none(), true);
  drop(res);
  assert_eq!(codec.is_empty(), true);
}

#[test]
fn test_codec_split_line() {
  let mut codec = LineCodec::new(10);
  let mut res = codec.add_bytes("Hello ".as_bytes());
  assert_eq!(res.next().is_none(), true);
  drop(res);
  let mut res = codec.add_bytes("world\n".as_bytes());
  assert_eq!(res.next().unwrap().as_str(), "Hello world");
  assert_eq!(res.next().is_none(), true);
  drop(res);
  assert_eq!(codec.is_empty(), true);
}

#[test]
fn test_codec_long_line() {
  let mut codec = LineCodec::new(10);
  let mut res = codec.add_bytes(&['a' as u8; 1000]);
  assert_eq!(
    res.next().unwrap().as_str(),
    std::str::from_utf8(&['a' as u8; 512]).unwrap()
  );
  assert_eq!(res.next().is_none(), true);
  drop(res);
  assert_eq!(codec.is_empty(), true);
}
