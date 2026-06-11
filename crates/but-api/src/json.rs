//! Shared JSON types and utilities to produce decent JSON from API types.
//!
//! This module is reserved for general-purpose transport helpers and JSON types
//! that are shared across API modules.
//!
//! If a JSON type only mirrors one API submodule, define it next to that API in
//! a local `json` module instead of adding it here. See `crate::branch::json`,
//! `crate::commit::json`, and `crate::diff::json` for the intended pattern.
pub use error::{Error, ToJsonError, UnmarkedError};
use gix::refs::Target;
use schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

mod hex_hash {
    use std::{ops::Deref, str::FromStr};

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    /// A type that deserializes a hexadecimal hash into an object id automatically.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct HexHash(pub gix::ObjectId);

    impl From<HexHash> for gix::ObjectId {
        fn from(value: HexHash) -> Self {
            value.0
        }
    }

    impl From<gix::ObjectId> for HexHash {
        fn from(value: gix::ObjectId) -> Self {
            HexHash(value)
        }
    }

    impl Deref for HexHash {
        type Target = gix::ObjectId;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<'de> Deserialize<'de> for HexHash {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let hex = String::deserialize(deserializer)?;
            gix::ObjectId::from_str(&hex)
                .map(HexHash)
                .map_err(serde::de::Error::custom)
        }
    }

    impl Serialize for HexHash {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self.0.to_hex().to_string())
        }
    }

    mod stringy {
        use std::str::FromStr;

        use schemars::JsonSchema;
        use serde::{Deserialize, Deserializer, Serialize, Serializer};

        /// A type that deserializes a hexadecimal hash into a string, unchanged.
        /// This is to workaround `schemars` which doesn't (always) work with transformations.
        #[derive(Debug, Clone, JsonSchema)]
        pub struct HexHashString(String);

        impl TryFrom<HexHashString> for gix::ObjectId {
            type Error = gix::hash::decode::Error;

            fn try_from(value: HexHashString) -> Result<Self, Self::Error> {
                value.0.parse()
            }
        }

        impl From<gix::ObjectId> for HexHashString {
            fn from(value: gix::ObjectId) -> Self {
                HexHashString(value.to_hex().to_string())
            }
        }

        impl<'de> Deserialize<'de> for HexHashString {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let hex = String::deserialize(deserializer)?;
                gix::ObjectId::from_str(&hex)
                    .map(|_| HexHashString(hex))
                    .map_err(serde::de::Error::custom)
            }
        }

        impl Serialize for HexHashString {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }
    }
    pub use stringy::HexHashString;

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn hex_hash() {
            let hex_str = "5c69907b1244089142905dba380371728e2e8160";
            let expected = gix::ObjectId::from_str(hex_str).expect("valid SHA1 hex-string");
            let actual =
                serde_json::from_str::<HexHash>(&format!("\"{hex_str}\"")).expect("input is valid");
            assert_eq!(actual.0, expected);

            let actual = serde_json::to_string(&actual);
            assert_eq!(
                actual.unwrap(),
                "\"5c69907b1244089142905dba380371728e2e8160\""
            );
        }
    }
}
pub use hex_hash::{HexHash, HexHashString};

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HexHashString);

/// Shared JSON transport type for mutation workspace results.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceState {
    /// Commits that were replaced by the operation. Maps `oldId -> newId`.
    #[cfg_attr(
        feature = "export-schema",
        schemars(with = "std::collections::BTreeMap<String, String>")
    )]
    pub replaced_commits: std::collections::BTreeMap<HexHash, HexHash>,
    /// The post-operation workspace view presented to the frontend.
    pub head_info: but_workspace::ui::RefInfo,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WorkspaceState);

impl TryFrom<crate::WorkspaceState> for WorkspaceState {
    type Error = anyhow::Error;

    fn try_from(
        crate::WorkspaceState {
            replaced_commits,
            head_info,
        }: crate::WorkspaceState,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            replaced_commits: replaced_commits
                .into_iter()
                .map(|(old, new)| (HexHash::from(old), HexHash::from(new)))
                .collect(),
            head_info: head_info.try_into()?,
        })
    }
}

mod error {
    //! Utilities to control which errors show in the frontend.
    //!
    //! ## How to use this
    //!
    //! Just make sure this `Error` type is used for each provided `tauri` command. The rest happens automatically
    //! such that [context](gitbutler_error::error::Context) is handled correctly.
    //!
    //! ### Interfacing with `tauri` using `Error`
    //!
    //! `tauri` serializes backend errors and makes these available as JSON objects to the frontend. The format
    //! is an implementation detail, but here it's implemented to turn each `Error` into a dict with `code`
    //! and `message` fields.
    //!
    //! The values in these fields are controlled by attaching context, please [see the `error` docs](gitbutler_error::error))
    //! on how to do this.

    use std::borrow::Cow;

    use but_error::AnyhowContextExt;
    use serde::{Serialize, ser::SerializeMap};

    /// An error type for serialization which isn't expected to carry a code.
    #[derive(Debug)]
    pub struct UnmarkedError(anyhow::Error);

    impl<T> From<T> for UnmarkedError
    where
        T: std::error::Error + Send + Sync + 'static,
    {
        fn from(err: T) -> Self {
            Self(err.into())
        }
    }

    impl Serialize for UnmarkedError {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let ctx = self.0.custom_context_or_error_chain();

            let mut map = serializer.serialize_map(Some(2))?;
            map.serialize_entry("code", &ctx.code.to_string())?;
            let message = ctx.message.unwrap_or_else(|| {
                self.0
                    .source()
                    .map(|err| Cow::Owned(err.to_string()))
                    .unwrap_or_else(|| Cow::Borrowed("Something went wrong"))
            });
            map.serialize_entry("message", &message)?;
            map.end()
        }
    }

    /// An error type for serialization, dynamically extracting context information during serialization,
    /// meant for consumption by the frontend.
    #[derive(Debug)]
    pub struct Error(anyhow::Error);

    impl From<anyhow::Error> for Error {
        fn from(value: anyhow::Error) -> Self {
            Self(value)
        }
    }

    impl From<Error> for anyhow::Error {
        fn from(value: Error) -> Self {
            value.0
        }
    }

    /// A utility to convert any `Result<T, impl std::error::Error>` into a [JSON-Error](Error).
    pub trait ToJsonError<T> {
        /// Convert this instance into a Result<T, [JSON-Error](Error)>.
        fn to_json_error(self) -> Result<T, Error>;
    }

    impl<T, E: std::error::Error + Send + Sync + 'static> ToJsonError<T> for Result<T, E> {
        fn to_json_error(self) -> Result<T, Error> {
            self.map_err(|e| Error(e.into()))
        }
    }

    impl Serialize for Error {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let ctx = self.0.custom_context_or_error_chain();

            let mut map = serializer.serialize_map(Some(2))?;
            map.serialize_entry("code", &ctx.code.to_string())?;
            let message = ctx.message.unwrap_or_else(|| {
                self.0
                    .source()
                    .map(|err| Cow::Owned(err.to_string()))
                    .unwrap_or_else(|| Cow::Borrowed("An unknown backend error occurred"))
            });
            map.serialize_entry("message", &message)?;
            map.end()
        }
    }

    #[cfg(test)]
    mod tests {
        use anyhow::anyhow;
        use but_error::{Code, Context};

        use super::*;

        fn json(err: anyhow::Error) -> String {
            serde_json::to_string(&Error(err)).unwrap()
        }

        #[test]
        fn no_context_or_code_shows_root_error() {
            let err = anyhow!("err msg");
            assert_eq!(
                format!("{err:#}"),
                "err msg",
                "just one error on display here"
            );
            assert_eq!(
                json(err),
                "{\"code\":\"Unknown\",\"message\":\"err msg\"}",
                "if there is no explicit error code or context, the original error message is shown (and chain)"
            );
        }

        #[test]
        fn find_code() {
            let err = anyhow!("err msg").context(Code::Validation);
            assert_eq!(
                format!("{err:#}"),
                "Validation: err msg",
                "note how the context becomes an error, in front of the original one"
            );
            assert_eq!(
                json(err),
                "{\"code\":\"Validation\",\"message\":\"err msg\"}",
                "the 'code' is available as string, but the message is taken from the source error"
            );
        }

        #[test]
        fn error_chain_display_without_context_or_code() {
            let original_err = std::io::Error::other("actual cause");
            let err = anyhow::Error::from(original_err).context("err msg");

            insta::assert_json_snapshot!(Error(err), @r#"
            {
              "code": "Unknown",
              "message": "err msg\n\nCaused by:\n    1: actual cause\n"
            }
            "#);
        }

        #[test]
        fn find_code_after_cause() {
            let original_err = std::io::Error::other("actual cause");
            let err = anyhow::Error::from(original_err)
                .context("err msg")
                .context(Code::Validation);

            assert_eq!(
                format!("{err:#}"),
                "Validation: err msg: actual cause",
                "an even longer chain, with the cause as root as one might expect"
            );
            assert_eq!(
                json(err),
                "{\"code\":\"Validation\",\"message\":\"err msg\"}",
                "in order to attach a custom message to an original cause, our messaging (and Code) is the tail"
            );
        }

        #[test]
        fn find_context() {
            let err = anyhow!("err msg").context(Context::new_static(Code::Validation, "ctx msg"));
            assert_eq!(format!("{err:#}"), "ctx msg: err msg");
            assert_eq!(
                json(err),
                "{\"code\":\"Validation\",\"message\":\"ctx msg\"}",
                "Contexts often provide their own message, so the error message is ignored"
            );
        }

        #[test]
        fn find_context_without_message() {
            let err = anyhow!("err msg").context(Context::from(Code::Validation));
            assert_eq!(
                format!("{err:#}"),
                "Something went wrong: err msg",
                "on display, `Context` does just insert a generic message"
            );
            assert_eq!(
                json(err),
                "{\"code\":\"Validation\",\"message\":\"err msg\"}",
                "Contexts without a message show the error's message as well"
            );
        }

        #[test]
        fn find_nested_code() {
            let err = anyhow!("bottom msg")
                .context("top msg")
                .context(Code::Validation);
            assert_eq!(
                format!("{err:#}"),
                "Validation: top msg: bottom msg",
                "now it's clear why bottom is bottom"
            );
            assert_eq!(
                json(err),
                "{\"code\":\"Validation\",\"message\":\"top msg\"}",
                "the 'code' gets the message of the error that it provides context to, and it finds it down the chain"
            );
        }

        #[test]
        fn multiple_codes() {
            let err = anyhow!("bottom msg")
                .context(Code::ProjectGitAuth)
                .context("top msg")
                .context(Code::Validation);
            assert_eq!(
                format!("{err:#}"),
                "Validation: top msg: ProjectGitAuth: bottom msg",
                "each code is treated like its own error in the chain"
            );
            assert_eq!(
                json(err),
                "{\"code\":\"Validation\",\"message\":\"top msg\"}",
                "it finds the most recent 'code' (and the same would be true for contexts, of course)"
            );
        }
    }
}

/// To make bstring work with schemars.
#[cfg(feature = "path-bytes")]
fn bstring_schema(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    // TODO: implement this. How to get description and what not?
    generate.root_schema_for::<String>()
}

/// The full name of a Git reference.
#[derive(Debug, Clone, schemars::JsonSchema, Serialize)]
pub struct FullRefName {
    /// The full name, like `refs/heads/main` or `refs/remotes/origin/foo`.
    /// Note that it might be degenerated if it can't be represented in Unicode.
    pub full: String,
    /// `full` without degeneration, as plain bytes.
    #[cfg(feature = "path-bytes")]
    #[schemars(schema_with = "but_schemars::bstring_bytes")]
    pub full_bytes: bstr::BString,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(FullRefName);

impl From<gix::refs::FullName> for FullRefName {
    fn from(value: gix::refs::FullName) -> Self {
        FullRefName {
            full: value.as_bstr().to_string(),
            #[cfg(feature = "path-bytes")]
            full_bytes: value.as_bstr().into(),
        }
    }
}

impl From<&gix::refs::FullNameRef> for FullRefName {
    fn from(value: &gix::refs::FullNameRef) -> Self {
        FullRefName {
            full: value.as_bstr().to_string(),
            #[cfg(feature = "path-bytes")]
            full_bytes: value.as_bstr().into(),
        }
    }
}

fn full_ref_from_bstring(value: bstr::BString) -> anyhow::Result<FullRefName> {
    Ok(gix::refs::FullName::try_from(value)?.into())
}

/// The full name of a Git reference, transported losslessly as bytes.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FullRefNameBytes {
    /// The full ref name, like `refs/heads/feat`, without UTF-8 loss.
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::bstring_bytes")
    )]
    pub full_name_bytes: bstr::BString,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(FullRefNameBytes);

impl<'de> Deserialize<'de> for FullRefNameBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct De {
            full_name_bytes: bstr::BString,
        }

        let value = De::deserialize(deserializer)?;
        gix::refs::FullName::try_from(value.full_name_bytes.clone())
            .map_err(serde::de::Error::custom)?;
        Ok(FullRefNameBytes {
            full_name_bytes: value.full_name_bytes,
        })
    }
}

impl TryFrom<FullRefNameBytes> for gix::refs::FullName {
    type Error = gix::refs::name::Error;

    fn try_from(value: FullRefNameBytes) -> Result<Self, Self::Error> {
        gix::refs::FullName::try_from(value.full_name_bytes)
    }
}

impl From<gix::refs::FullName> for FullRefNameBytes {
    fn from(value: gix::refs::FullName) -> Self {
        FullRefNameBytes {
            full_name_bytes: value.into_inner(),
        }
    }
}

impl From<FullRefNameBytes> for String {
    fn from(value: FullRefNameBytes) -> Self {
        gix::refs::FullName::try_from(value)
            .map(|name| name.shorten().to_string())
            .expect("FullRefNameBytes deserialization validates ref names")
    }
}

/// A Git reference identified by its full reference name, along with the information Git stores about it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Reference {
    /// The full name, like `refs/heads/main` or `refs/remotes/origin/foo`.
    /// Note that it might be degenerated if it can't be represented in Unicode.
    pub name: FullRefName,
    /// Set if the reference points to an object id. This is the common case.
    #[serde(default)]
    pub target_id: Option<HexHashString>,
    /// Set if the reference points to the name of another reference. This happens if the reference is symbolic.
    #[serde(default)]
    pub target_ref: Option<FullRefName>,
}

impl From<gix::refs::Reference> for Reference {
    fn from(
        gix::refs::Reference {
            name,
            target,
            peeled: _ignored,
        }: gix::refs::Reference,
    ) -> Self {
        Reference {
            name: name.into(),
            target_id: match &target {
                Target::Object(id) => Some((*id).into()),
                Target::Symbolic(_) => None,
            },
            target_ref: match target {
                Target::Object(_) => None,
                Target::Symbolic(rn) => Some(rn.into()),
            },
        }
    }
}

/// A reference in `refs/heads`, with its full name for API use and short name for display.
#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FullBranchReference {
    /// The full ref name, like `refs/heads/feat`, for usage with the backend.
    pub full_name: FullRefName,
    /// The short version of `full_name` for display.
    pub display_name: String,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(FullBranchReference);

impl From<but_workspace::ui::ref_info::BranchReference> for FullBranchReference {
    fn from(value: but_workspace::ui::ref_info::BranchReference) -> Self {
        FullBranchReference {
            full_name: full_ref_from_bstring(value.full_name_bytes)
                .expect("BranchReference stores a validated full ref name"),
            display_name: value.display_name,
        }
    }
}

/// A reference in `refs/remotes`, with its full name for API use and short name for display.
#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FullRemoteTrackingReference {
    /// The full ref name, like `refs/remotes/origin/on-remote`, for usage with the backend.
    pub full_name: FullRefName,
    /// The short version of `full_name` for display, like `on-remote`, without the remote name.
    pub display_name: String,
    /// The symbolic name of the remote, like `origin`, or `origin/other`.
    pub remote_name: String,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(FullRemoteTrackingReference);

impl From<but_workspace::ui::ref_info::RemoteTrackingReference> for FullRemoteTrackingReference {
    fn from(value: but_workspace::ui::ref_info::RemoteTrackingReference) -> Self {
        FullRemoteTrackingReference {
            full_name: full_ref_from_bstring(value.full_name_bytes)
                .expect("RemoteTrackingReference stores a validated full ref name"),
            display_name: value.display_name,
            remote_name: value.remote_name,
        }
    }
}

/// Information about the target reference, the one we want to integrate with.
#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TargetRefInfo {
    /// The remote tracking branch of the target to integrate with, like `refs/remotes/origin/main`.
    pub remote_tracking_ref: FullRemoteTrackingReference,
    /// The amount of commits that aren't reachable by any segment in the workspace, they are in its future.
    pub commits_ahead: usize,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(TargetRefInfo);

impl From<but_workspace::ui::ref_info::Target> for TargetRefInfo {
    fn from(value: but_workspace::ui::ref_info::Target) -> Self {
        TargetRefInfo {
            remote_tracking_ref: value.remote_tracking_ref.into(),
            commits_ahead: value.commits_ahead,
        }
    }
}

/// A segment of a commit graph, representing a set of commits exclusively.
#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RefInfoSegment {
    /// The name of the branch that denotes this segment, if available.
    pub ref_name: Option<FullBranchReference>,
    /// The name of the remote tracking branch of this segment, if present.
    pub remote_tracking_ref_name: Option<FullRemoteTrackingReference>,
    /// The portion of commits that can be reached from the tip of the branch downwards.
    pub commits: Vec<but_workspace::ui::Commit>,
    /// Commits that are reachable from the remote-tracking branch associated with this branch.
    pub commits_on_remote: Vec<but_workspace::ui::UpstreamCommit>,
    /// All commits that are not workspace commits reachable by this segment.
    pub commits_outside: Option<Vec<but_workspace::ui::Commit>>,
    /// Read-only metadata with additional information about the branch naming the segment.
    pub metadata: Option<but_core::ref_metadata::Branch>,
    /// Whether this segment is the traversal entrypoint.
    pub is_entrypoint: bool,
    /// A derived value to help the UI decide which functions to make available.
    pub push_status: but_workspace::ui::PushStatus,
    /// The base commit that this segment rests on, if available.
    #[serde(with = "but_serde::object_id_opt")]
    #[schemars(schema_with = "but_schemars::object_id_opt")]
    pub base: Option<gix::ObjectId>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(RefInfoSegment);

impl From<but_workspace::ui::ref_info::Segment> for RefInfoSegment {
    fn from(value: but_workspace::ui::ref_info::Segment) -> Self {
        RefInfoSegment {
            ref_name: value.ref_name.map(Into::into),
            remote_tracking_ref_name: value.remote_tracking_ref_name.map(Into::into),
            commits: value.commits,
            commits_on_remote: value.commits_on_remote,
            commits_outside: value.commits_outside,
            metadata: value.metadata,
            is_entrypoint: value.is_entrypoint,
            push_status: value.push_status,
            base: value.base,
        }
    }
}

/// The UI-clone of `branch::Stack`.
#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RefInfoStack {
    /// The stack identifier if this segment belongs to GitButler stack metadata.
    #[schemars(schema_with = "but_schemars::stack_id_opt")]
    pub id: Option<but_core::ref_metadata::StackId>,
    /// The base commit shared by the stack, if available.
    #[serde(with = "but_serde::object_id_opt")]
    #[schemars(schema_with = "but_schemars::object_id_opt")]
    pub base: Option<gix::ObjectId>,
    /// The branch-name denoted segments of the stack from its tip to the point of reference.
    pub segments: Vec<RefInfoSegment>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(RefInfoStack);

impl From<but_workspace::ui::ref_info::Stack> for RefInfoStack {
    fn from(value: but_workspace::ui::ref_info::Stack) -> Self {
        RefInfoStack {
            id: value.id,
            base: value.base,
            segments: value.segments.into_iter().map(Into::into).collect(),
        }
    }
}

/// The current workspace ref information returned to N-API callers.
#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HeadInfo {
    /// The ref that points to a workspace commit, or the first stack segment ref.
    pub workspace_ref: Option<FullBranchReference>,
    /// The stacks visible in the current workspace.
    pub stacks: Vec<RefInfoStack>,
    /// The target to integrate workspace stacks into.
    pub target: Option<TargetRefInfo>,
    /// Whether the workspace ref belongs to GitButler metadata.
    pub is_managed_ref: bool,
    /// Whether the workspace points to a GitButler-created workspace commit.
    pub is_managed_commit: bool,
    /// Whether the workspace represents what `HEAD` is pointing to.
    pub is_entrypoint: bool,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HeadInfo);

impl TryFrom<but_workspace::RefInfo> for HeadInfo {
    type Error = anyhow::Error;

    fn try_from(value: but_workspace::RefInfo) -> Result<Self, Self::Error> {
        but_workspace::ui::RefInfo::try_from(value).map(Into::into)
    }
}

impl From<but_workspace::ui::RefInfo> for HeadInfo {
    fn from(value: but_workspace::ui::RefInfo) -> Self {
        HeadInfo {
            workspace_ref: value.workspace_ref.map(Into::into),
            stacks: value.stacks.into_iter().map(Into::into).collect(),
            target: value.target.map(Into::into),
            is_managed_ref: value.is_managed_ref,
            is_managed_commit: value.is_managed_commit,
            is_entrypoint: value.is_entrypoint,
        }
    }
}

/// Information about the current state of a branch.
#[derive(Serialize, Debug, Clone, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BranchDetailsWithFullRefName {
    /// The short name of the branch, like `foo` for `refs/heads/foo`.
    pub name: String,
    /// The full reference of the branch.
    pub reference: FullRefName,
    /// The id of the linked worktree that has the branch checked out.
    pub linked_worktree_id: Option<String>,
    /// Upstream reference, e.g. `refs/remotes/origin/base-branch-improvements`.
    pub remote_tracking_branch: Option<FullRefName>,
    /// The pull request associated with the branch.
    pub pr_number: Option<usize>,
    /// A unique identifier for the GitButler review associated with the branch, if any.
    pub review_id: Option<String>,
    /// The tip commit of the branch.
    #[serde(with = "but_serde::object_id")]
    #[schemars(schema_with = "but_schemars::object_id")]
    pub tip: gix::ObjectId,
    /// The base commit from the perspective of this branch.
    #[serde(with = "but_serde::object_id")]
    #[schemars(schema_with = "but_schemars::object_id")]
    pub base_commit: gix::ObjectId,
    /// The pushable status for the branch.
    pub push_status: but_workspace::ui::PushStatus,
    /// Last time the branch was updated in Epoch milliseconds.
    pub last_updated_at: Option<i128>,
    /// All authors of the commits in the branch.
    pub authors: Vec<but_workspace::ui::Author>,
    /// Whether the branch is conflicted.
    pub is_conflicted: bool,
    /// The commits contained in the branch, excluding the upstream commits.
    pub commits: Vec<but_workspace::ui::Commit>,
    /// The commits that are only at the remote.
    pub upstream_commits: Vec<but_workspace::ui::UpstreamCommit>,
    /// Whether this branch details view represents a remote head.
    pub is_remote_head: bool,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(BranchDetailsWithFullRefName);

impl TryFrom<but_workspace::ui::BranchDetails> for BranchDetailsWithFullRefName {
    type Error = anyhow::Error;

    fn try_from(value: but_workspace::ui::BranchDetails) -> Result<Self, Self::Error> {
        Ok(BranchDetailsWithFullRefName {
            name: value.name.to_string(),
            reference: value.reference.into(),
            linked_worktree_id: value.linked_worktree_id.map(|id| id.to_string()),
            remote_tracking_branch: value
                .remote_tracking_branch
                .map(full_ref_from_bstring)
                .transpose()?,
            pr_number: value.pr_number,
            review_id: value.review_id,
            tip: value.tip,
            base_commit: value.base_commit,
            push_status: value.push_status,
            last_updated_at: value.last_updated_at,
            authors: value.authors,
            is_conflicted: value.is_conflicted,
            commits: value.commits,
            upstream_commits: value.upstream_commits,
            is_remote_head: value.is_remote_head,
        })
    }
}
