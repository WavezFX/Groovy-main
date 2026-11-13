// Kokoro's weights in a file of their own, mapped instead of copied (round 3D
// item 7). Loaded from the downloaded model.int8.onnx, onnxruntime copies its
// 111 MB of weights into Groovy's private memory. Moved to a side file, each
// at a 64 KB boundary, onnxruntime maps them from disk instead: shared,
// read-only pages Windows can drop and reread, which the private working set
// (Task Manager's "Memory") doesn't count. Same weights, same voice.
//
// The ONNX file is a protobuf: ModelProto.graph (7) → GraphProto.initializer
// (5) → TensorProto.raw_data (9). Each large raw_data is moved out and
// replaced by external_data (13: location, offset, length) and data_location
// (14) = EXTERNAL. Every other field is copied as it is.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Smaller tensors stay in the model file.
const MIN_BYTES: usize = 64 * 1024;
/// Windows maps files at its allocation granularity.
const ALIGN: u64 = 64 * 1024;

fn varint(buf: &[u8], pos: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let b = *buf.get(*pos)?;
        *pos += 1;
        v |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Some(v);
        }
    }
    None
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn put_len(out: &mut Vec<u8>, field: u32, bytes: &[u8]) {
    put_varint(out, u64::from(field) << 3 | 2);
    put_varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

/// One field: its number, its wire type, the whole field as written, and for
/// a length-delimited field its contents.
struct Field<'a> {
    number: u32,
    whole: &'a [u8],
    body: Option<&'a [u8]>,
}

fn fields(buf: &[u8]) -> Option<Vec<Field<'_>>> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < buf.len() {
        let start = pos;
        let tag = varint(buf, &mut pos)?;
        let body = match tag & 7 {
            0 => {
                varint(buf, &mut pos)?;
                None
            }
            1 => {
                pos += 8;
                None
            }
            2 => {
                let len = varint(buf, &mut pos)? as usize;
                let b = buf.get(pos..pos.checked_add(len)?)?;
                pos += len;
                Some(b)
            }
            5 => {
                pos += 4;
                None
            }
            _ => return None,
        };
        out.push(Field { number: (tag >> 3) as u32, whole: buf.get(start..pos)?, body });
    }
    Some(out)
}

/// The model with its large weights moved into `data` (written to `data_out`
/// from offset 0), each referred to by `location`.
fn split(model: &[u8], location: &str, data_out: &mut impl Write) -> Option<Vec<u8>> {
    let mut offset = 0u64;
    let mut out = Vec::with_capacity(model.len() / 16);
    let mut moved = 0;
    for f in fields(model)? {
        let (7, Some(graph)) = (f.number, f.body) else {
            out.extend_from_slice(f.whole);
            continue;
        };
        let mut g = Vec::new();
        for gf in fields(graph)? {
            let (5, Some(tensor)) = (gf.number, gf.body) else {
                g.extend_from_slice(gf.whole);
                continue;
            };
            let tf = fields(tensor)?;
            let raw = tf.iter().find(|t| t.number == 9).and_then(|t| t.body);
            // Already external: it has external_data (a data_location of
            // DEFAULT, written out by some exporters, doesn't count).
            let external = tf.iter().any(|t| t.number == 13);
            let Some(raw) = raw.filter(|r| r.len() >= MIN_BYTES && !external) else {
                g.extend_from_slice(gf.whole);
                continue;
            };
            let pad = (ALIGN - offset % ALIGN) % ALIGN;
            data_out.write_all(&vec![0u8; pad as usize]).ok()?;
            offset += pad;
            data_out.write_all(raw).ok()?;
            let mut t = Vec::new();
            for x in tf.iter().filter(|t| t.number != 9 && t.number != 14) {
                t.extend_from_slice(x.whole);
            }
            for (key, value) in [("location", location.to_string()), ("offset", offset.to_string()), ("length", raw.len().to_string())] {
                let mut entry = Vec::new();
                put_len(&mut entry, 1, key.as_bytes());
                put_len(&mut entry, 2, value.as_bytes());
                put_len(&mut t, 13, &entry);
            }
            put_varint(&mut t, 14 << 3);
            put_varint(&mut t, 1);
            offset += raw.len() as u64;
            moved += 1;
            put_len(&mut g, 5, &t);
        }
        put_len(&mut out, 7, &g);
    }
    (moved > 0).then_some(out)
}

/// The mapped twin of `model` (written next to it the first time), or None
/// if it can't be made: Kokoro then loads the original.
pub fn mapped_model(model: &Path) -> Option<PathBuf> {
    let dir = model.parent()?;
    let stem = model.file_stem()?.to_str()?;
    let twin = dir.join(format!("{stem}.mapped.onnx"));
    let data_name = format!("{stem}.mapped.data");
    if twin.is_file() && dir.join(&data_name).is_file() {
        return Some(twin);
    }
    let started = std::time::Instant::now();
    let source = std::fs::read(model).ok()?;
    let tmp_data = dir.join(format!("{data_name}.part"));
    let mut data = std::io::BufWriter::new(std::fs::File::create(&tmp_data).ok()?);
    let made = split(&source, &data_name, &mut data).and_then(|m| data.flush().ok().map(|_| m));
    drop(data);
    let Some(made) = made else {
        let _ = std::fs::remove_file(&tmp_data);
        crate::log::line("Kokoro: couldn't map its weights, loading them whole");
        return None;
    };
    let tmp_twin = dir.join(format!("{stem}.mapped.onnx.part"));
    let ok = std::fs::write(&tmp_twin, made).is_ok() && std::fs::rename(&tmp_data, dir.join(&data_name)).is_ok() && std::fs::rename(&tmp_twin, &twin).is_ok();
    if !ok {
        let _ = std::fs::remove_file(&tmp_data);
        let _ = std::fs::remove_file(&tmp_twin);
        return None;
    }
    crate::log::line(format!("Kokoro: weights mapped from disk ({:?})", started.elapsed()));
    Some(twin)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tensor(name: &str, raw: &[u8]) -> Vec<u8> {
        let mut t = Vec::new();
        put_varint(&mut t, 1 << 3);
        put_varint(&mut t, 4);
        put_len(&mut t, 8, name.as_bytes());
        put_len(&mut t, 9, raw);
        t
    }

    #[test]
    fn a_default_data_location_is_replaced_not_kept() {
        let mut t = tensor("w", &vec![1u8; MIN_BYTES]);
        put_varint(&mut t, 14 << 3);
        put_varint(&mut t, 0);
        let mut graph = Vec::new();
        put_len(&mut graph, 5, &t);
        let mut model = Vec::new();
        put_len(&mut model, 7, &graph);
        let mut data = Vec::new();
        let out = split(&model, "w.data", &mut data).unwrap();
        assert_eq!(data.len(), MIN_BYTES);
        let g = fields(fields(&out).unwrap()[0].body.unwrap()).unwrap();
        let locs: Vec<&[u8]> = fields(g[0].body.unwrap()).unwrap().iter().filter(|f| f.number == 14).map(|f| f.whole).collect();
        assert_eq!(locs, [&[14u8 << 3, 1][..]], "one data_location, EXTERNAL");
    }

    #[test]
    fn large_weights_move_out_aligned_and_the_rest_stays() {
        let big_a = vec![7u8; MIN_BYTES + 3];
        let big_b = vec![9u8; MIN_BYTES];
        let mut graph = Vec::new();
        put_len(&mut graph, 1, b"a node");
        put_len(&mut graph, 5, &tensor("a", &big_a));
        put_len(&mut graph, 5, &tensor("small", &[1, 2, 3]));
        put_len(&mut graph, 5, &tensor("b", &big_b));
        let mut model = Vec::new();
        put_varint(&mut model, 1 << 3);
        put_varint(&mut model, 8);
        put_len(&mut model, 7, &graph);
        let mut data = Vec::new();
        let out = split(&model, "w.data", &mut data).unwrap();

        assert_eq!(&data[..big_a.len()], &big_a[..]);
        assert_eq!(&data[ALIGN as usize * 2..], &big_b[..], "the second starts at the next 64 KB boundary");
        let top = fields(&out).unwrap();
        assert_eq!(top[0].whole, &model[..2], "ir_version kept");
        let g = fields(top[1].body.unwrap()).unwrap();
        assert_eq!(g[0].body, Some(&b"a node"[..]));
        let small = fields(g[2].body.unwrap()).unwrap();
        assert_eq!(small.iter().find(|f| f.number == 9).unwrap().body, Some(&[1u8, 2, 3][..]), "a small tensor stays inline");
        let b = fields(g[3].body.unwrap()).unwrap();
        assert!(b.iter().all(|f| f.number != 9), "raw data moved out");
        let entries: Vec<(String, String)> = b
            .iter()
            .filter(|f| f.number == 13)
            .map(|f| {
                let e = fields(f.body.unwrap()).unwrap();
                (String::from_utf8(e[0].body.unwrap().to_vec()).unwrap(), String::from_utf8(e[1].body.unwrap().to_vec()).unwrap())
            })
            .collect();
        assert_eq!(entries, [("location".into(), "w.data".into()), ("offset".into(), (ALIGN * 2).to_string()), ("length".into(), MIN_BYTES.to_string())]);
        let loc = b.iter().find(|f| f.number == 14).unwrap();
        assert_eq!(loc.whole, &[14 << 3, 1], "data_location = EXTERNAL");
    }

    #[test]
    fn a_broken_or_weightless_model_is_left_alone() {
        assert!(split(&[0x3a, 0x7f, 1], "w", &mut Vec::new()).is_none(), "a length past the end");
        let mut model = Vec::new();
        put_len(&mut model, 7, &[]);
        assert!(split(&model, "w", &mut Vec::new()).is_none(), "nothing to move");
    }
}
