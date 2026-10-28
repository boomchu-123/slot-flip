use slot::thumb;
use slot_retro::{GBA_H, GBA_W};

fn decode(png_bytes: &[u8]) -> (Vec<u8>, u32, u32) {
    let mut reader = png::Decoder::new(png_bytes).read_info().expect("read info");
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("decode");
    assert_eq!(info.color_type, png::ColorType::Rgb);
    buf.truncate(info.buffer_size());
    (buf, info.width, info.height)
}

#[test]
fn a_thumbnail_keeps_the_frames_colours() {
    let mut frame = vec![0u8; (GBA_W * GBA_H * 4) as usize];
    for px in frame.chunks_exact_mut(4) {
        px.copy_from_slice(&[0x20, 0x40, 0xd0, 0xff]);
    }
    let encoded = thumb::png(&frame).expect("encode");
    let (rgb, w, h) = decode(&encoded);
    assert_eq!((w, h), (GBA_W, GBA_H));
    assert_eq!(&rgb[..3], &[0xd0, 0x40, 0x20]);
}

#[test]
fn a_short_frame_is_not_a_thumbnail() {
    assert!(thumb::png(&[]).is_none());
    assert!(thumb::png(&vec![0u8; (GBA_W * GBA_H * 4) as usize - 4]).is_none());
}

#[test]
fn a_thumbnail_of_a_larger_picture_keeps_its_size() {
    let (w, h) = (320u32, 240u32);
    let mut frame = vec![0u8; (w * h * 4) as usize];
    for px in frame.chunks_exact_mut(4) {
        px.copy_from_slice(&[0x20, 0x40, 0xd0, 0xff]);
    }
    let encoded = thumb::png_sized(&frame, w, h).expect("encode");
    let (rgb, dw, dh) = decode(&encoded);
    assert_eq!((dw, dh), (w, h));
    assert_eq!(&rgb[..3], &[0xd0, 0x40, 0x20]);
}

#[test]
fn a_sized_thumbnail_refuses_a_buffer_that_is_too_short_or_empty() {
    assert!(thumb::png_sized(&[0u8; 16], 320, 240).is_none());
    assert!(thumb::png_sized(&[0u8; 16], 0, 0).is_none());
}
