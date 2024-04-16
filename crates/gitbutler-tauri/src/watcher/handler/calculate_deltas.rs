use anyhow::{Context, Result};
use gitbutler_core::{
    deltas, gb_repository, project_repository, projects::ProjectId, reader, sessions,
};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use tracing::instrument;

impl super::Handler {
    #[instrument(skip(self, paths, project_id))]
    pub fn calculate_deltas(&self, paths: Vec<PathBuf>, project_id: ProjectId) -> Result<()> {
        let make_processor = || -> Result<_> {
            let project = self
                .projects
                .get(&project_id)
                .context("failed to get project")?;
            let project_repository = project_repository::Repository::open(&project)
                .with_context(|| "failed to open project repository for project")?;
            let user = self.users.get_user().context("failed to get user")?;
            let gb_repository = gb_repository::Repository::open(
                &self.local_data_dir,
                &project_repository,
                user.as_ref(),
            )
            .context("failed to open gb repository")?;

            // If current session's branch is not the same as the project's head, flush it first.
            if let Some(session) = gb_repository
                .get_current_session()
                .context("failed to get current session")?
            {
                let project_head = project_repository
                    .get_head()
                    .context("failed to get head")?;
                if session.meta.branch != project_head.name().map(|n| n.to_string()) {
                    gb_repository
                        .flush_session(&project_repository, &session, user.as_ref())
                        .context(format!("failed to flush session {}", session.id))?;
                }
            }
            let current_session = gb_repository
                .get_or_create_current_session()
                .context("failed to get or create current session")?;
            let session = current_session.clone();

            let process = move |path: &Path| -> Result<bool> {
                let _span = tracing::span!(tracing::Level::TRACE, "processing", ?path).entered();
                let current_session_reader =
                    sessions::Reader::open(&gb_repository, &current_session)
                        .context("failed to get session reader")?;
                let deltas_reader = deltas::Reader::new(&current_session_reader);
                let writer =
                    deltas::Writer::new(&gb_repository).context("failed to open deltas writer")?;
                let current_wd_file_content = match Self::file_content(&project_repository, path) {
                    Ok(content) => Some(content),
                    Err(reader::Error::NotFound) => None,
                    Err(err) => Err(err).context("failed to get file content")?,
                };
                let latest_file_content = match current_session_reader.file(path) {
                    Ok(content) => Some(content),
                    Err(reader::Error::NotFound) => None,
                    Err(err) => Err(err).context("failed to get file content")?,
                };
                let current_deltas = deltas_reader
                    .read_file(path)
                    .context("failed to get file deltas")?;
                let mut text_doc = deltas::Document::new(
                    latest_file_content.as_ref(),
                    current_deltas.unwrap_or_default(),
                )?;
                let new_delta = text_doc
                    .update(current_wd_file_content.as_ref())
                    .context("failed to calculate new deltas")?;

                let Some(new_delta) = new_delta else {
                    return Ok(false);
                };

                let deltas = text_doc.get_deltas();
                writer
                    .write(path, &deltas)
                    .context("failed to write deltas")?;

                match &current_wd_file_content {
                    Some(reader::Content::UTF8(text)) => writer.write_wd_file(path, text),
                    Some(_) => writer.write_wd_file(path, ""),
                    None => writer.remove_wd_file(path),
                }?;

                let session_id = current_session.id;
                self.emit_session_file(project_id, session_id, path, latest_file_content.as_ref())?;
                self.index_deltas(
                    project_id,
                    session_id,
                    path,
                    std::slice::from_ref(&new_delta),
                )
                .context("failed to index deltas")?;
                self.emit_app_event(&crate::events::Event::deltas(
                    project_id,
                    session_id,
                    std::slice::from_ref(&new_delta),
                    path,
                ))?;
                Ok(true)
            };
            Ok((process, session))
        };
        let num_paths = paths.len();
        let num_no_delta = std::thread::scope(|scope| -> Result<usize> {
            let num_threads = std::thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(1).unwrap())
                .get()
                .min(paths.len());
            let mut num_no_delta = 0;
            let current_session = if num_threads < 2 {
                let (process, session) = make_processor()?;
                for path in paths {
                    if !process(path.as_path())? {
                        num_no_delta += 1;
                    }
                }
                session
            } else {
                let (threads, tx) = {
                    let (tx, rx) = crossbeam_channel::bounded::<PathBuf>(num_threads);
                    let threads: Vec<_> = (0..num_threads)
                        .map(|id| {
                            std::thread::Builder::new()
                                .name(format!("gitbutler_delta_thread_{id}"))
                                .stack_size(512 * 1024)
                                .spawn_scoped(scope, {
                                    let rx = rx.clone();
                                    || -> Result<usize> {
                                        let mut num_no_delta = 0;
                                        let (process, _) = make_processor()?;
                                        for path in rx {
                                            if !process(path.as_path())? {
                                                num_no_delta += 1;
                                            }
                                        }
                                        Ok(num_no_delta)
                                    }
                                })
                                .expect("worker thread can be created")
                        })
                        .collect();
                    (threads, tx)
                };
                for path in paths {
                    tx.send(path).expect("many receivers");
                }
                drop(tx);

                for thread in threads {
                    num_no_delta += thread.join().unwrap()?;
                }
                let (_, session) = make_processor()?;
                session
            };
            self.index_session(project_id, &current_session)?;
            Ok(num_no_delta)
        })?;
        tracing::debug!(%project_id, paths_without_deltas = num_no_delta, paths_with_delta = num_paths - num_no_delta);
        Ok(())
    }

    fn file_content(
        project_repository: &project_repository::Repository,
        path: &Path,
    ) -> Result<reader::Content, reader::Error> {
        let full_path = project_repository.project().path.join(path);
        if !full_path.exists() {
            return Err(reader::Error::NotFound);
        }
        Ok(reader::Content::read_from_file(&full_path)?)
    }
}
