use super::buffer::*;

#[test]
pub fn test_buffer_basic() {
  let mut pool = BufferPool::new(512, 10);
  // No buffers should be allocated to start.
  assert_eq!(pool.size_buffers(), 0);

  let a = pool
    .take()
    .expect("should be able to allocate first buffer");
  let id_a = identity(&a);

  assert_eq!(pool.size_buffers(), 1);
  drop(a);

  let b = pool
    .take()
    .expect("should be able to allocate a buffer again");
  let id_b = identity(&b);

  // Since the first Buffer was dropped, it should have been returned to the pool and the second
  // request should not cause the pool's size to increase.
  assert_eq!(pool.size_buffers(), 1);

  // The two buffers should be the same.
  assert_eq!(id_a, id_b);
}

#[test]
pub fn test_buffer_several() {
  let mut pool = BufferPool::new(512, 10);
  // No buffers should be allocated to start.
  assert_eq!(pool.size_buffers(), 0);

  let a = pool
    .take()
    .expect("should be able to allocate first buffer");
  let id_a = identity(&a);

  let b = pool
    .take()
    .expect("should be able to allocate second buffer");
  let id_b = identity(&b);

  // The pool should now be two buffers large.
  assert_eq!(pool.size_buffers(), 2);

  // And those buffers should be different.
  assert_ne!(id_a, id_b);

  // Return the first buffer to the pool.
  drop(a);

  let c = pool
    .take()
    .expect("should be able to allocate a buffer again");
  let id_c = identity(&c);

  // Since the first Buffer was dropped, it should have been returned to the pool and the second
  // request should not cause the pool's size to increase.
  assert_eq!(pool.size_buffers(), 2);

  // The new buffer should be the same as the buffer that was dropped.
  assert_eq!(id_a, id_c);
}

fn identity(buf: &Buffer) -> *const str {
  buf.as_str() as *const str
}
