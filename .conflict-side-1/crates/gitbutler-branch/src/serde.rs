use gitbutler_diff::HunkHash;
use serde::Serialize;

pub fn hash_to_hex<S>(v: &HunkHash, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    format!("{v:x}").serialize(s)
}
