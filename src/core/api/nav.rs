//! Safe navigation into InnerTube JSON. Path segments that parse as numbers index arrays.
use serde_json::Value;

pub fn nav<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = v;
    for seg in path {
        cur = match seg.parse::<usize>() {
            Ok(i) if cur.is_array() => cur.get(i)?,
            _ => cur.get(*seg)?,
        };
    }
    Some(cur)
}

pub fn nav_str<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    nav(v, path)?.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn walks_objects_and_arrays() {
        let v = json!({"a": [{"b": "x"}, {"b": "y"}]});
        assert_eq!(nav_str(&v, &["a", "1", "b"]), Some("y"));
        assert!(nav(&v, &["a", "5"]).is_none());
        assert!(nav(&v, &["zzz", "b"]).is_none());
    }
}
