pub(crate) trait RelaPath {
    fn rela_path(&self) -> &bstr::BStr;
}

impl RelaPath for gix::diff::index::ChangeRef<'_, '_> {
    fn rela_path(&self) -> &bstr::BStr {
        match self {
            gix::diff::index::ChangeRef::Addition { location, .. }
            | gix::diff::index::ChangeRef::Modification { location, .. }
            | gix::diff::index::ChangeRef::Rewrite { location, .. }
            | gix::diff::index::ChangeRef::Deletion { location, .. } => location,
        }
    }
}
