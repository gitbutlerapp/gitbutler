//! A replayable repository archive backed by a Git bundle.

use std::{fmt::Write as _, io, path::Path};

use anyhow::{Context as _, Result, ensure};
use but_core::RepositoryExt as _;
use gix::bstr::ByteSlice as _;

use crate::{
    args::{Args, RepoInstallerArgs},
    setup,
};

use super::{
    ArchiveEntry, ArchiveWriter, DumpProgress, OutputPath, ProgressWriter, acquire_archive_lock,
    archive_base_name, default_output_path, effective_current_dir, entry_name,
    generated_executable_options, open_archive_dir_unless_requested, persist_archive,
    portable_file_options, sorted_children, stored_file_options,
};

/// Execute the `dump repo-installer` subcommand.
pub(super) fn run(
    args: &Args,
    installer_args: &RepoInstallerArgs,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<()> {
    let current_dir = effective_current_dir(args)?;
    let repo = setup::repo_from_args(args).with_context(|| {
        format!(
            "Could not discover Git repository at '{}'",
            current_dir.display()
        )
    })?;
    let repo = gix::open_opts(repo.git_dir().canonicalize()?, repo.open_options().clone())?;
    let workdir = repo
        .workdir()
        .context("Repository installers require a non-bare repository")?;
    ensure!(
        repo.git_dir() == repo.common_dir(),
        "Repository installers require the main worktree"
    );

    let project_data_dir = repo
        .gitbutler_storage_path()
        .context("Could not locate GitButler project data")?;
    ensure!(
        project_data_dir.is_dir(),
        "GitButler project data does not exist at '{}'",
        project_data_dir.display()
    );

    let base = archive_base_name(&repo)?;
    let archive_root = format!("{base}-repo-installer");
    let output_path = match &installer_args.archive.output {
        Some(path) => current_dir.join(path),
        None => default_output_path(&repo, "repo-installer")?,
    };
    let output_path = OutputPath::new(output_path, current_dir);
    let archive_lock = acquire_archive_lock(&output_path.path, Some(workdir.to_owned()))?;
    let output_path = output_path.with_lock_path(archive_lock.lock_path().to_owned());

    let _repo_guard =
        but_core::sync::exclusive_repo_access(repo.git_dir(), Some(&project_data_dir));
    let clone_remote = clone_remote(&repo, workdir)?;
    let (head_oid, head_ref) = head(&repo)?;
    let config = selected_local_config(&repo)?;
    let (packed_refs, symrefs) = ref_manifests(&repo)?;

    let bundle_dir = tempfile::tempdir().context("Could not create temporary bundle directory")?;
    let bundle_path = bundle_dir.path().join("repository.bundle");
    writeln!(out, "Creating repository delta bundle")?;
    create_bundle(&repo, &bundle_path)?;
    let script = installer_script(&base, &clone_remote, &head_oid, head_ref.as_deref());

    let progress = DumpProgress::new()?;
    let file = ProgressWriter::new(archive_lock, &progress);
    let mut archive = ArchiveWriter::new(file, &progress);
    archive.add_directory(format!("{archive_root}/"))?;
    archive.add_generated_file_with_options(
        format!("{archive_root}/install.sh"),
        script.as_bytes(),
        generated_executable_options(),
    )?;
    archive.add_file_with_options(
        &bundle_path,
        format!("{archive_root}/repository.bundle"),
        stored_file_options(),
    )?;
    archive.add_generated_file_with_options(
        format!("{archive_root}/packed-refs"),
        &packed_refs,
        portable_file_options(),
    )?;
    archive.add_generated_file_with_options(
        format!("{archive_root}/git-config"),
        &config,
        portable_file_options(),
    )?;
    archive.add_generated_file_with_options(
        format!("{archive_root}/symrefs"),
        &symrefs,
        portable_file_options(),
    )?;
    add_project_data(&mut archive, &project_data_dir, &archive_root, &output_path)?;
    let file = archive.finish(err)?;
    persist_archive(file.into_inner())?;

    writeln!(out, "Archive at: {}", output_path.path.display())?;
    open_archive_dir_unless_requested(
        &output_path.path,
        installer_args.archive.no_open_archive_directory,
    )?;
    Ok(())
}

struct CloneRemote {
    name: String,
    url: String,
}

fn clone_remote(repo: &gix::Repository, workdir: &Path) -> Result<CloneRemote> {
    let remote_names = repo.remote_names();
    let project_meta = but_core::ref_metadata::ProjectMeta::resolve(repo)?;
    let target_remote = project_meta
        .remote_target_ref(repo)?
        .as_ref()
        .and_then(|target| {
            but_core::extract_remote_name_and_short_name(target.as_ref(), &remote_names)
                .map(|(name, _branch)| name)
        });
    let name = match target_remote {
        Some(name) if repo.find_remote(name.as_str()).is_ok() => name,
        _ => repo
            .remote_default_name(gix::remote::Direction::Fetch)
            .context("Could not determine a clone remote")?
            .to_str()
            .context("Clone remote name is not UTF-8")?
            .to_owned(),
    };
    let remote = repo
        .find_remote(name.as_str())
        .with_context(|| format!("Could not find clone remote '{name}'"))?;
    let remote_url = remote
        .url(gix::remote::Direction::Fetch)
        .with_context(|| format!("Clone remote '{name}' has no fetch URL"))?;
    let mut url = remote_url
        .to_bstring()
        .to_str()
        .context("Clone remote URL is not UTF-8")?
        .to_owned();
    if remote_url.scheme == gix::url::Scheme::File
        && remote_url.serialize_alternative_form
        && Path::new(&url).is_relative()
    {
        url = workdir
            .join(&url)
            .to_str()
            .context("Absolute clone remote path is not UTF-8")?
            .to_owned();
    }
    ensure!(
        !url.contains(['\n', '\r', '\0']),
        "Clone remote URL contains unsupported control characters"
    );
    Ok(CloneRemote { name, url })
}

fn head(repo: &gix::Repository) -> Result<(String, Option<String>)> {
    let head = repo.head().context("Could not read HEAD")?;
    let oid = head.id().context("Repository HEAD is unborn")?.to_string();
    let reference = head
        .referent_name()
        .map(|name| {
            name.as_bstr()
                .to_str()
                .context("HEAD reference name is not UTF-8")
                .map(ToOwned::to_owned)
        })
        .transpose()?;
    Ok((oid, reference))
}

fn create_bundle(repo: &gix::Repository, bundle_path: &Path) -> Result<()> {
    // Release the reference iterator's packed-ref lock before following symbolic refs.
    let references = repo.references()?.all()?.collect::<gix::Result<Vec<_>>>()?;
    let mut selected = Vec::with_capacity(references.len() + 1);
    let mut excluded = Vec::new();
    for mut reference in references {
        let name = reference.name().to_owned();
        let object_id = match reference.follow_to_object() {
            Ok(id) => id.detach(),
            Err(err)
                if err
                    .downcast_any_ref::<gix::refs::file::find::NotFound>()
                    .is_some() =>
            {
                continue;
            }
            Err(err) => return Err(err.into()),
        };
        if name.as_bstr().starts_with(b"refs/remotes/") {
            excluded.push(object_id);
        }
        selected.push((name, object_id));
    }
    selected.push((
        "HEAD".try_into()?,
        repo.find_reference("HEAD")?.follow_to_object()?.detach(),
    ));
    let mut output = io::BufWriter::new(
        std::fs::File::create(bundle_path).context("Could not create repository bundle file")?,
    );
    repo.write_bundle(
        selected,
        excluded,
        &mut output,
        gix::progress::Discard,
        &gix::interrupt::IS_INTERRUPTED,
    )
    .context("Could not create repository bundle")?;
    io::Write::flush(&mut output).context("Could not finish repository bundle")?;
    Ok(())
}

fn selected_local_config(repo: &gix::Repository) -> Result<Vec<u8>> {
    use gix::config::{File, Source, file, path::interpolate};

    let git_dir = repo.current_dir().join(repo.git_dir());
    let workdir = repo.current_dir().join(
        repo.workdir()
            .context("Repository installers require a non-bare repository")?,
    );
    let head_name = repo.head_name()?;
    let home = gix::path::env::home_dir()
        .and_then(|path| repo.open_options().permissions.env.home.check_opt(path));
    let install_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_owned));
    // Resolve includes within local scope: global remote URLs must not enable hasconfig conditions.
    let config = File::from_paths_metadata_buf(
        &mut [file::Metadata::from(Source::Local)
            .at(repo.current_dir().join(repo.common_dir()).join("config"))
            .with(repo.git_dir_trust())]
        .into_iter(),
        &mut Vec::new(),
        false,
        file::init::Options {
            includes: file::includes::Options::follow(
                interpolate::Context {
                    git_install_dir: install_dir.as_deref(),
                    home_dir: home.as_deref(),
                    home_for_user: Some(interpolate::home_for_user),
                },
                file::includes::conditional::Context {
                    git_dir: Some(&git_dir),
                    branch_name: head_name.as_ref().map(AsRef::as_ref),
                },
            )
            .strict(),
            lossy: true,
            ..Default::default()
        },
    )
    .context("Could not read repository-local Git configuration")?
    .unwrap_or_default();

    let mut selected = Vec::new();
    for section in config.sections() {
        let header = section.header();
        let section_name = header.name().to_ascii_lowercase();
        if !matches!(section_name.as_slice(), b"gitbutler" | b"remote" | b"log") {
            continue;
        }
        for (mut name, value) in section.body().into_iter_implicit() {
            name.make_ascii_lowercase();
            if section_name == b"log"
                && (header.subsection_name().is_some() || name != "excludedecoration")
            {
                continue;
            }
            let value = value.context("Git configuration record did not contain a value")?;
            let mut key = section_name.clone();
            if let Some(subsection) = header.subsection_name() {
                key.push(b'.');
                key.extend_from_slice(subsection);
            }
            key.push(b'.');
            key.extend_from_slice(name.as_bytes());
            if is_storage_path_key(&key) {
                continue;
            }
            selected.extend_from_slice(&key);
            selected.push(b'\n');
            if is_remote_url_key(&key)
                && let Some(url) = absolute_file_url(&value, &workdir)
            {
                selected.extend_from_slice(&url);
            } else {
                selected.extend_from_slice(&value);
            }
            selected.push(0);
        }
    }
    Ok(selected)
}

fn is_remote_url_key(key: &[u8]) -> bool {
    key.len() > b"remote..url".len()
        && key[..b"remote.".len()].eq_ignore_ascii_case(b"remote.")
        && key[key.len() - b".url".len()..].eq_ignore_ascii_case(b".url")
}

fn absolute_file_url(value: &[u8], workdir: &Path) -> Option<Vec<u8>> {
    let url = gix::url::parse(value).ok()?;
    if url.scheme != gix::url::Scheme::File || !url.serialize_alternative_form {
        return None;
    }
    let path = gix::path::from_bstr(value.as_bstr()).ok()?;
    if !path.is_relative() {
        return None;
    }
    gix::path::into_bstr(workdir.join(path))
        .ok()?
        .as_ref()
        .to_vec()
        .into()
}

fn is_storage_path_key(key: &[u8]) -> bool {
    [
        b"gitbutler.storagepath".as_slice(),
        b"gitbutler.nightly.storagepath".as_slice(),
        b"gitbutler.dev.storagepath".as_slice(),
    ]
    .iter()
    .any(|candidate| key.eq_ignore_ascii_case(candidate))
}

fn ref_manifests(repo: &gix::Repository) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut direct_refs = Vec::new();
    let mut symbolic_refs = Vec::new();
    for reference in repo.references()?.all()? {
        let reference = reference.map_err(|err| anyhow::anyhow!(err.to_string()))?;
        let target = reference.target();
        if let Some(oid) = target.try_id() {
            direct_refs.push((reference.name().as_bstr().to_owned(), oid.to_string()));
        } else if let Some(target) = target.try_name() {
            symbolic_refs.push((
                reference.name().as_bstr().to_owned(),
                target.as_bstr().to_owned(),
            ));
        }
    }
    direct_refs.sort();
    symbolic_refs.sort();

    let mut packed = b"# pack-refs with: sorted\n".to_vec();
    for (name, oid) in direct_refs {
        packed.extend_from_slice(oid.as_bytes());
        packed.push(b' ');
        packed.extend_from_slice(&name);
        packed.push(b'\n');
    }

    let mut symbolic = Vec::new();
    for (name, target) in symbolic_refs {
        symbolic.extend_from_slice(&name);
        symbolic.push(b'\t');
        symbolic.extend_from_slice(&target);
        symbolic.push(b'\n');
    }
    Ok((packed, symbolic))
}

fn installer_script(
    base: &str,
    remote: &CloneRemote,
    head_oid: &str,
    head_ref: Option<&str>,
) -> String {
    let mut script = String::from("#!/usr/bin/env bash\nset -euo pipefail\n\n");
    writeln!(script, "repository_name={}", shell_quote(base)).expect("writing to String works");
    writeln!(script, "clone_remote={}", shell_quote(&remote.name))
        .expect("writing to String works");
    writeln!(script, "clone_url={}", shell_quote(&remote.url)).expect("writing to String works");
    writeln!(script, "head_oid={}", shell_quote(head_oid)).expect("writing to String works");
    writeln!(
        script,
        "head_ref={}",
        shell_quote(head_ref.unwrap_or_default())
    )
    .expect("writing to String works");
    script.push_str(INSTALLER_BODY);
    script
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

const INSTALLER_BODY: &str = r#"
unset $(git rev-parse --local-env-vars)

root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
destination="$root/$repository_name-clone"
bundle="$root/repository.bundle"
packed_refs="$root/packed-refs"
state="$root/gitbutler"
config="$root/git-config"
symrefs="$root/symrefs"

step() {
    printf '\n==> %s\n' "$1"
}

if [[ ! -f "$bundle" ]]; then
    echo "Required repository bundle is missing: $bundle" >&2
    exit 1
fi

if [[ -e "$destination" ]]; then
    echo "Refusing to overwrite existing destination: $destination" >&2
    exit 1
fi

step "Cloning repository into $destination"
git -c init.defaultRefFormat=files clone --progress --no-checkout --origin "$clone_remote" -- "$clone_url" "$destination"

step "Restoring repository configuration"
git -C "$destination" remote | while IFS= read -r remote; do
    git -C "$destination" remote remove "$remote"
done

for mode in unset add; do
    while IFS= read -r -d '' record; do
        key=${record%%$'\n'*}
        value=${record#*$'\n'}
        if [[ "$mode" == unset ]]; then
            git -C "$destination" config --local --unset-all "$key" || true
        else
            git -C "$destination" config --local --add "$key" "$value"
        fi
    done <"$config"
done

step "Fetching remote prerequisite objects"
git -C "$destination" remote | while IFS= read -r remote; do
    if [[ "$remote" != "$clone_remote" ]]; then
        git -C "$destination" fetch --progress --no-tags "$remote"
    fi
done

step "Verifying repository delta bundle"
git -C "$destination" bundle verify "$bundle" >/dev/null
step "Importing local Git objects"
git -C "$destination" bundle unbundle --progress "$bundle" >/dev/null

step "Restoring repository refs"
git_dir=$(git -C "$destination" rev-parse --absolute-git-dir)
rm -rf -- "$git_dir/refs" "$git_dir/packed-refs"
mkdir -p "$git_dir/refs"
cp "$packed_refs" "$git_dir/packed-refs"
while IFS=$'\t' read -r ref target; do
    if [[ -n "$ref" && -n "$target" ]]; then
        git -C "$destination" symbolic-ref "$ref" "$target"
    fi
done <"$symrefs"

step "Restoring HEAD and worktree"
if git -C "$destination" cat-file -e "$head_oid^{commit}" 2>/dev/null; then
    git -C "$destination" checkout --quiet --detach --force "$head_oid"
else
    echo "Warning: saved HEAD object $head_oid is unavailable; leaving the worktree empty." >&2
fi
if [[ -n "$head_ref" ]]; then
    git -C "$destination" symbolic-ref HEAD "$head_ref"
else
    printf '%s\n' "$head_oid" >"$git_dir/HEAD"
fi

step "Restoring GitButler state"
rm -rf -- "$git_dir/gitbutler"
cp -R "$state" "$git_dir/gitbutler"
for key in gitbutler.storagePath gitbutler.nightly.storagePath gitbutler.dev.storagePath; do
    git -C "$destination" config --local --replace-all "$key" gitbutler
done

printf '\nRepository restored at: %s\n' "$destination"
"#;

fn add_project_data<W: io::Write + io::Seek>(
    archive: &mut ArchiveWriter<'_, W>,
    project_data_dir: &Path,
    archive_root: &str,
    output_path: &OutputPath,
) -> Result<()> {
    let state_root = format!("{archive_root}/gitbutler");
    archive.add_directory(format!("{state_root}/"))?;
    let mut stack = sorted_children(project_data_dir)?;
    while let Some(path) = stack.pop() {
        if output_path.is_same(&path) {
            continue;
        }
        let relative = path.strip_prefix(project_data_dir)?;
        let entry_name = entry_name(&state_root, relative).with_context(|| {
            format!(
                "GitButler state path cannot be represented in the archive: '{}'",
                relative.display()
            )
        })?;
        let meta = path.symlink_metadata()?;
        let is_dir = meta.is_dir();
        archive.add_entry_with_compression(
            ArchiveEntry {
                path: path.clone(),
                entry_name,
                meta,
            },
            zip::CompressionMethod::Deflated,
        )?;
        if is_dir {
            stack.extend(sorted_children(&path)?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use but_testsupport::gix_testtools;

    use super::*;

    #[test]
    fn bundle_creation_does_not_execute_git() -> gix_testtools::TestResult {
        if gix_testtools::run_in_isolated_process()? {
            return Ok(());
        }
        let fixture =
            gix_testtools::scripted_fixture_read_only("dump-repo-installer.sh")?.canonicalize()?;
        let no_programs = tempfile::tempdir()?;
        let output = tempfile::tempdir()?;
        let _environment =
            gix_testtools::Env::new().set("PATH", no_programs.path().display().to_string());
        let repo = gix::open_opts(fixture.join("gitbutler"), gix::open::Options::isolated())?;
        create_bundle(&repo, &output.path().join("repository.bundle"))?;
        Ok(())
    }

    #[test]
    fn selected_config_matches_git_without_executing_git() -> gix_testtools::TestResult {
        if gix_testtools::run_in_isolated_process()? {
            return Ok(());
        }
        let fixture = gix_testtools::scripted_fixture_read_only("dump-repo-installer-config.sh")?
            .canonicalize()?;
        let no_programs = tempfile::tempdir()?;
        let _environment = gix_testtools::Env::new()
            .set(
                "GIT_CONFIG_GLOBAL",
                fixture.join("global.config").display().to_string(),
            )
            .set("PATH", no_programs.path().display().to_string());
        let repo = gix::open(fixture.join("values"))?;
        assert!(
            repo.config_snapshot()
                .strings("gitbutler.included")
                .is_some_and(|values| values.iter().any(|value| value == "global-url")),
            "the effective config includes values enabled by a global remote URL"
        );
        assert_eq!(
            selected_local_config(&repo)?,
            std::fs::read(fixture.join("expected-config"))?,
            "export matches Git's local scope, include order, duplicate values, key spelling, and decoded bytes"
        );
        Ok(())
    }

    #[test]
    fn selected_config_absolutizes_relative_fetch_urls() -> gix_testtools::TestResult {
        let fixture = gix_testtools::scripted_fixture_read_only("dump-repo-installer-config.sh")?
            .canonicalize()?;
        let workdir = fixture.join("paths");
        let mut expected = b"remote.Relative.url\n".to_vec();
        expected.extend_from_slice(gix::path::into_bstr(workdir.join("../remote.git"))?.as_ref());
        expected.extend_from_slice(
            b"\0remote.Relative.pushurl\n../push.git\0remote.Absolute.url\n/absolute/remote.git\0",
        );
        assert_eq!(
            selected_local_config(&gix::open_opts(&workdir, gix::open::Options::isolated())?)?,
            expected,
            "relative fetch paths become absolute while push URLs and existing absolute paths remain intact"
        );
        Ok(())
    }

    #[test]
    fn selected_config_rejects_implicit_values_and_accepts_no_matches() -> gix_testtools::TestResult
    {
        let fixture = gix_testtools::scripted_fixture_read_only("dump-repo-installer-config.sh")?
            .canonicalize()?;
        for name in ["implicit-first", "implicit-last", "implicit-storage"] {
            assert!(
                selected_local_config(&gix::open_opts(
                    fixture.join(name),
                    gix::open::Options::isolated()
                )?)
                .is_err(),
                "{name}: implicit values cannot be replayed as explicit empty values"
            );
        }
        assert!(
            selected_local_config(&gix::open_opts(
                fixture.join("empty"),
                gix::open::Options::isolated()
            )?)?
            .is_empty(),
            "a repository with no selected values produces an empty manifest"
        );
        Ok(())
    }

    #[test]
    fn shell_quotes_values() {
        assert_eq!(shell_quote("plain"), "'plain'", "plain text is quoted");
        assert_eq!(
            shell_quote("it's $HOME"),
            "'it'\"'\"'s $HOME'",
            "quotes are escaped without expanding variables"
        );
    }

    #[test]
    fn identifies_storage_path_keys() {
        assert!(
            is_storage_path_key(b"gitbutler.storagepath"),
            "release storage path is identified"
        );
        assert!(
            is_storage_path_key(b"gitbutler.nightly.storagePath"),
            "channel storage paths are identified case-insensitively"
        );
        assert!(
            !is_storage_path_key(b"gitbutler.project.targetref"),
            "project metadata must remain in the archive"
        );
    }
}
