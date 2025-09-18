use bstr::{BStr, BString, ByteVec};
use std::collections::{BTreeSet, HashMap};
use std::ptr;

/// A lookup table as a tree, with trees represented by their path component.
pub struct Lut {
    nodes: Vec<TreeNode>,
}

impl Default for Lut {
    fn default() -> Self {
        Lut {
            nodes: vec![TreeNode {
                children: Default::default(),
            }],
        }
    }
}

#[allow(clippy::indexing_slicing)]
impl Lut {
    /// Insert a node for each component in slash-separated `rela_path`.
    pub fn track_file(&mut self, rela_path: &BStr) {
        let mut next_index = self.nodes.len();
        let mut cursor = &mut self.nodes[0];
        for component in to_components(rela_path) {
            match cursor.children.get(component).copied() {
                None => {
                    cursor.children.insert(component.to_owned(), next_index);
                    self.nodes.push(TreeNode::default());
                    cursor = &mut self.nodes[next_index];
                    next_index += 1;
                }
                Some(existing_idx) => {
                    cursor = &mut self.nodes[existing_idx];
                }
            }
        }
    }

    /// Given `rela_path`, place all leaf paths into `out` which is partially or fully
    /// intersecting with `rela_path`.
    /// Note that all paths in `out` will be `/` separated relative paths.
    ///
    /// For example, in a tree with paths `a`, `b/c` and `b/d`, `rela_path` of `a/b` will match `a`,
    /// while `b` would match `b/c` and `b/d`.
    pub fn get_intersecting(&self, rela_path: &BStr, out: &mut BTreeSet<BString>) {
        let mut cur_path = BString::default();
        let mut cursor = &self.nodes[0];

        for component in to_components(rela_path) {
            match cursor.children.get(component).copied() {
                None => {
                    if !cur_path.is_empty() && cursor.children.is_empty() {
                        out.insert(cur_path.clone());
                    }
                    return;
                }
                Some(existing_idx) => {
                    cursor = &self.nodes[existing_idx];
                    push_component(&mut cur_path, component)
                }
            }
        }

        if cursor.children.is_empty() || ptr::eq(cursor, &self.nodes[0]) {
            if !cur_path.is_empty() {
                out.insert(cur_path.clone());
            }
            return;
        }

        let mut queue: Vec<_> = cursor
            .children
            .iter()
            .map(|(component, idx)| (cur_path.len(), (component, *idx)))
            .collect();

        while let Some((cur_path_len, (component, idx))) = queue.pop() {
            cur_path.truncate(cur_path_len);
            push_component(&mut cur_path, component.as_ref());

            let node = &self.nodes[idx];
            if node.children.is_empty() {
                if !cur_path.is_empty() {
                    out.insert(cur_path.clone());
                }
            } else {
                queue.extend(
                    node.children
                        .iter()
                        .map(|(component, idx)| (cur_path.len(), (component, *idx))),
                );
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
struct TreeNode {
    /// A mapping of path component names (always without slash) to their entry in the [`Lut::nodes`].
    /// Note that the node is only a leaf if it has no `children` itself, which is when it can be assumed to be a file.
    children: HashMap<BString, usize>,
}

pub fn to_components(rela_path: &BStr) -> impl Iterator<Item = &BStr> {
    rela_path.split(|b| *b == b'/').map(Into::into)
}

pub fn push_component(path: &mut BString, component: &BStr) {
    if !path.is_empty() {
        path.push_byte(b'/');
    }
    path.extend_from_slice(component);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journey() {
        let mut lut = Lut::default();
        for path in ["a", "b/c", "b/d"] {
            lut.track_file(path.into());
        }
        let mut out = BTreeSet::new();
        lut.get_intersecting("".into(), &mut out);
        assert_eq!(out.len(), 0, "empty means nothing, instead of everything");

        lut.get_intersecting("d".into(), &mut out);
        // This one could not be found.
        insta::assert_debug_snapshot!(out, @r"{}");

        lut.get_intersecting("a".into(), &mut out);
        // Perfect match
        insta::assert_compact_debug_snapshot!(out, @r#"{"a"}"#);

        out.clear();
        lut.get_intersecting("a/b".into(), &mut out);
        // indirect match, prefix, single item
        insta::assert_compact_debug_snapshot!(out, @r#"{"a"}"#);

        out.clear();
        lut.get_intersecting("b".into(), &mut out);
        // indirect match, suffix/leafs
        insta::assert_debug_snapshot!(out, @r#"
            {
                "b/c",
                "b/d",
            }
            "#);

        out.clear();
        lut.get_intersecting("b/x/y".into(), &mut out);
        // No match, nothing in the way.
        insta::assert_debug_snapshot!(out, @r#"{}"#);
    }

    #[test]
    fn complex_journey() {
        let mut lut = Lut::default();
        for path in [
            "a/1/2/4/5",
            "a/1/2/3",
            "a/2/3",
            "a/3",
            "b/3/4/5",
            "b/3/2/1",
            "b/2/3",
            "b/1",
        ] {
            lut.track_file(path.into());
        }
        let mut out = BTreeSet::new();

        lut.get_intersecting("a".into(), &mut out);
        insta::assert_debug_snapshot!(out, @r#"
            {
                "a/1/2/3",
                "a/1/2/4/5",
                "a/2/3",
                "a/3",
            }
            "#);

        // It's additive
        lut.get_intersecting("b".into(), &mut out);
        insta::assert_debug_snapshot!(out, @r#"
            {
                "a/1/2/3",
                "a/1/2/4/5",
                "a/2/3",
                "a/3",
                "b/1",
                "b/2/3",
                "b/3/2/1",
                "b/3/4/5",
            }
            "#);
    }
}
