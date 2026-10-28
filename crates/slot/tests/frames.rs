use slot::frames::Frames;

fn publish(frames: &Frames, value: u8, size: usize) {
    let mut buf = frames.take_write();
    buf.clear();
    buf.resize(size, value);
    frames.publish(buf);
}

#[test]
fn the_reader_gets_the_newest_frame_and_never_the_same_one_twice() {
    let f = Frames::new(4);
    for i in 0..50u8 {
        publish(&f, i, 4);
    }
    let latest = f.latest().expect("a published frame should be readable");
    assert_eq!(
        &latest[..],
        &[49u8; 4],
        "the reader was handed a stale frame"
    );
    drop(latest);
    assert!(
        f.latest().is_none(),
        "a consumed frame was handed out twice"
    );
}

#[test]
fn buffers_are_recycled_rather_than_reallocated_per_frame() {
    let f = Frames::new(4);
    publish(&f, 1, 4);
    let held = f.latest().expect("a published frame should be readable");
    for i in 0..500u32 {
        publish(&f, i as u8, 4);
    }
    drop(held);
    publish(&f, 0, 4);
    assert!(
        f.allocated() <= 3,
        "{} buffers in flight, the pool is leaking",
        f.allocated()
    );
}

#[test]
fn a_frame_published_without_a_size_is_the_gba_canvas() {
    let f = Frames::new(4);
    publish(&f, 7, 4);
    let latest = f.latest().expect("a published frame should be readable");
    assert_eq!(latest.size(), (slot_retro::GBA_W, slot_retro::GBA_H));
}

#[test]
fn a_frame_carries_the_size_it_was_published_at() {
    let f = Frames::new(4);
    let mut buf = f.take_write();
    buf.clear();
    buf.resize(320 * 240 * 4, 1);
    f.publish_sized(buf, (320, 240));
    let latest = f.latest().expect("a published frame should be readable");
    assert_eq!(latest.size(), (320, 240));
    assert_eq!(latest.len(), 320 * 240 * 4);
}

#[test]
fn the_newest_frame_brings_its_own_size_even_when_it_replaces_an_unread_one() {
    let f = Frames::new(4);
    let mut a = f.take_write();
    a.clear();
    a.resize(8, 1);
    f.publish_sized(a, (2, 1));
    let mut b = f.take_write();
    b.clear();
    b.resize(16, 2);
    f.publish_sized(b, (2, 2));
    let latest = f.latest().expect("a published frame should be readable");
    assert_eq!(
        (latest.size(), latest[0]),
        ((2, 2), 2),
        "stale size or data"
    );
}
