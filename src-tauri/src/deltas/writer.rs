use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::{
    gb_repository, reader,
    virtual_branches::{self, branch},
    writer::{self, Writer},
};

use super::{Delta, Reader};

pub struct DeltasWriter<'writer> {
    repository: &'writer gb_repository::Repository,
    writer: writer::DirWriter,
}

impl<'writer> DeltasWriter<'writer> {
    pub fn new(repository: &'writer gb_repository::Repository) -> Self {
        let writer = writer::DirWriter::open(repository.root());
        Self { writer, repository }
    }

    // TODO: right now deltas here must be a full list of current deltas for the file.
    // maybe we should change it to only write the deltas that are new?
    pub fn write<P: AsRef<std::path::Path>>(&self, path: P, deltas: &Vec<Delta>) -> Result<()> {
        self.repository
            .get_or_create_current_session()
            .context("failed to create session")?;

        self.repository.lock()?;
        defer! {
            self.repository.unlock().unwrap();
        }

        let path = path.as_ref();
        let raw_deltas = serde_json::to_string(&deltas)?;

        self.writer
            .write_string(&format!("session/deltas/{}", path.display()), &raw_deltas)?;

        log::info!(
            "{}: wrote deltas for {}",
            self.repository.project_id,
            path.display()
        );

        for (vbid, deltas) in self
            .split_virtual_branches(path, deltas)
            .context("failed to split into virtual branches")?
        {
            let raw_deltas = serde_json::to_string(&deltas)?;
            self.writer
                .write_string(
                    &format!("branches/{}/deltas/{}", vbid, path.display()),
                    &raw_deltas,
                )
                .context("failed to write virtual branch deltas")?;
            log::info!(
                "{}: wrote deltas for virtual branch {} for {}",
                self.repository.project_id,
                vbid,
                path.display()
            );
        }

        Ok(())
    }

    // returns a map of virtual branch id -> updated deltas list
    fn split_virtual_branches<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        deltas: &Vec<Delta>,
    ) -> Result<HashMap<String, Vec<Delta>>> {
        let dir_reader = reader::DirReader::open(self.repository.root());

        // first, read all virtual branches
        let mut virtual_branches = virtual_branches::Iterator::new(&dir_reader)
            .context("failed to read virtual branches")?
            .collect::<Result<Vec<branch::Branch>, crate::reader::Error>>()
            .context("failed to read virtual branches")?
            .into_iter()
            .filter(|branch| branch.applied)
            .collect::<Vec<_>>();

        // can't split into virtual branches if there are none
        if virtual_branches.is_empty() {
            return Ok(HashMap::new());
        }

        // read deltas for all virtual branches
        let mut vbranch_deltas: HashMap<String, Vec<Delta>> = HashMap::new();
        let delta_reader = Reader::new(&dir_reader);
        for branch in &virtual_branches {
            if let Some(deltas) = delta_reader
                .read_virtual_file(&branch.id, path.as_ref())
                .context("failed to read virtual branch deltas")?
            {
                vbranch_deltas.insert(branch.id.clone(), deltas);
            }
        }

        // sort virtual branches by id to make sure the order is deterministic for the fallback's
        // fallback choice
        virtual_branches.sort_by_key(|branch| branch.id.clone());

        // choose fallback virtual branch. it's either the selected one or just the first one
        let vbranch_reader = branch::Reader::new(&dir_reader);
        let fallback_branch_id = if let Some(id) = vbranch_reader
            .read_selected()
            .context("failed to read selected branch id")?
        {
            id
        } else {
            virtual_branches[0].id.clone()
        };

        // split every delta into viratual branches
        let mut new_deltas_by_vbranch = HashMap::new();
        for delta in deltas {
            let mut remaining = Some(delta.clone());
            for vbranch in &virtual_branches {
                let vb_deltas = if let Some(deltas) = vbranch_deltas.get(&vbranch.id) {
                    deltas
                } else {
                    continue;
                };

                // skip if delta is already taken
                if vb_deltas.contains(delta) {
                    remaining = None;
                    break;
                }

                for vb_delta in vb_deltas {
                    let taken_remaining = vb_delta.take(delta);
                    // if delta was taken by an existing virtual delta, add it to the result
                    if let Some(taken) = taken_remaining.0 {
                        let new_deltas = new_deltas_by_vbranch
                            .entry(vbranch.id.clone())
                            .or_insert_with(|| vb_deltas.clone()); // initialize with existing deltas
                        new_deltas.push(taken);
                    }

                    // update remaining delta to try to split it further
                    remaining = taken_remaining.1;

                    if remaining.is_none() {
                        break;
                    }
                }

                if remaining.is_none() {
                    break;
                }
            }

            // add the remaining delta to the fallback branch
            if let Some(deltas) = remaining {
                let new_deltas = new_deltas_by_vbranch
                    .entry(fallback_branch_id.clone())
                    .or_insert_with(Vec::new);
                new_deltas.push(deltas);
            }
        }

        Ok(new_deltas_by_vbranch)
    }

    pub fn write_wd_file<P: AsRef<std::path::Path>>(&self, path: P, contents: &str) -> Result<()> {
        self.repository
            .get_or_create_current_session()
            .context("failed to create session")?;

        self.repository.lock()?;
        defer! {
            self.repository.unlock().expect("failed to unlock");
        }

        let path = path.as_ref();
        self.writer
            .write_string(&format!("session/wd/{}", path.display()), contents)?;

        log::info!(
            "{}: wrote session wd file {}",
            self.repository.project_id,
            path.display()
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use tempfile::tempdir;

    use crate::{deltas, projects, sessions, storage, users};

    use super::*;

    fn test_repository() -> Result<git2::Repository> {
        let path = tempdir()?.path().to_str().unwrap().to_string();
        let repository = git2::Repository::init(path)?;
        let mut index = repository.index()?;
        let oid = index.write_tree()?;
        let signature = git2::Signature::now("test", "test@email.com").unwrap();
        repository.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &repository.find_tree(oid)?,
            &[],
        )?;
        Ok(repository)
    }

    static mut TEST_INDEX: usize = 0;

    fn test_branch() -> branch::Branch {
        unsafe {
            TEST_INDEX += 1;
        }
        branch::Branch {
            id: format!("branch_{}", unsafe { TEST_INDEX }),
            name: format!("branch_name_{}", unsafe { TEST_INDEX }),
            applied: true,
            upstream: format!("upstream_{}", unsafe { TEST_INDEX }),
            created_timestamp_ms: unsafe { TEST_INDEX } as u128,
            updated_timestamp_ms: unsafe { TEST_INDEX + 100 } as u128,
        }
    }

    #[test]
    fn write_no_vbranches() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let deltas_writer = DeltasWriter::new(&gb_repo);

        let session = gb_repo.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repo, &session)?;
        let deltas_reader = Reader::new(&session_reader);

        let path = "test.txt";
        let deltas = vec![
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((0, "hello".to_string()))],
                timestamp_ms: 0,
            },
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((5, " world".to_string()))],
                timestamp_ms: 0,
            },
        ];

        deltas_writer.write(path, &deltas).unwrap();

        assert_eq!(deltas_reader.read_file(path).unwrap(), Some(deltas));
        assert_eq!(deltas_reader.read_file("not found").unwrap(), None);
        assert_eq!(
            deltas_reader.read_virtual_file("not found", path).unwrap(),
            None
        );

        Ok(())
    }

    #[test]
    fn write_single_vbranch() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let deltas_writer = DeltasWriter::new(&gb_repo);
        let branch_writer = branch::Writer::new(&gb_repo);

        let vbranch0 = test_branch();
        branch_writer.write(&vbranch0)?;

        let session = gb_repo.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repo, &session)?;
        let deltas_reader = Reader::new(&session_reader);

        let path = "test.txt";
        let deltas = vec![
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((0, "hello".to_string()))],
                timestamp_ms: 0,
            },
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((5, " world".to_string()))],
                timestamp_ms: 0,
            },
        ];

        deltas_writer.write(path, &deltas).unwrap();

        assert_eq!(deltas_reader.read_file(path).unwrap(), Some(deltas.clone()));
        assert_eq!(
            deltas_reader.read_virtual_file(&vbranch0.id, path).unwrap(),
            Some(deltas)
        );

        Ok(())
    }

    #[test]
    fn write_distribute_into_multiple_vbranches() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let deltas_writer = DeltasWriter::new(&gb_repo);
        let branch_writer = branch::Writer::new(&gb_repo);

        let vbranch0 = test_branch();
        branch_writer.write(&vbranch0)?;
        let vbranch1 = test_branch();
        branch_writer.write(&vbranch1)?;

        let session = gb_repo.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repo, &session)?;
        let deltas_reader = Reader::new(&session_reader);

        let path = "test.txt";

        let deltas = vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "hello".to_string()))],
            timestamp_ms: 0,
        }];
        branch_writer.write_selected(Some(&vbranch1.id))?;
        deltas_writer.write(path, &deltas).unwrap();
        assert_eq!(deltas_reader.read_file(path).unwrap(), Some(deltas.clone()));
        assert_eq!(
            deltas_reader.read_virtual_file(&vbranch1.id, path).unwrap(),
            Some(deltas)
        );

        let deltas = vec![
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((0, "hello".to_string()))],
                timestamp_ms: 0,
            },
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((5, "world".to_string()))],
                timestamp_ms: 1,
            },
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((11, "!".to_string()))],
                timestamp_ms: 2,
            },
        ];
        branch_writer.write_selected(Some(&vbranch0.id))?;
        deltas_writer.write(path, &deltas).unwrap();
        assert_eq!(deltas_reader.read_file(path).unwrap(), Some(deltas.clone()));
        assert_eq!(
            deltas_reader.read_virtual_file(&vbranch0.id, path).unwrap(),
            Some(vec![deltas[2].clone()])
        );
        assert_eq!(
            deltas_reader.read_virtual_file(&vbranch1.id, path).unwrap(),
            Some(vec![deltas[0].clone(), deltas[1].clone()])
        );

        Ok(())
    }

    #[test]
    fn write_split_delta_into_multiple_vbranches() -> Result<()> {
        let repository = test_repository()?;
        let project = projects::Project::try_from(&repository)?;
        let gb_repo_path = tempdir()?.path().to_str().unwrap().to_string();
        let storage = storage::Storage::from_path(tempdir()?.path());
        let user_store = users::Storage::new(storage.clone());
        let project_store = projects::Storage::new(storage);
        project_store.add_project(&project)?;
        let gb_repo =
            gb_repository::Repository::open(gb_repo_path, project.id, project_store, user_store)?;

        let deltas_writer = DeltasWriter::new(&gb_repo);
        let branch_writer = branch::Writer::new(&gb_repo);

        let vbranch0 = test_branch();
        branch_writer.write(&vbranch0)?;
        let vbranch1 = test_branch();
        branch_writer.write(&vbranch1)?;

        let session = gb_repo.get_or_create_current_session()?;
        let session_reader = sessions::Reader::open(&gb_repo, &session)?;
        let deltas_reader = Reader::new(&session_reader);

        let path = "test.txt";

        let deltas = vec![deltas::Delta {
            operations: vec![deltas::Operation::Insert((0, "hello".to_string()))],
            timestamp_ms: 0,
        }];
        branch_writer.write_selected(Some(&vbranch1.id))?;
        deltas_writer.write(path, &deltas).unwrap();
        assert_eq!(deltas_reader.read_file(path).unwrap(), Some(deltas.clone()));
        assert_eq!(
            deltas_reader.read_virtual_file(&vbranch1.id, path).unwrap(),
            Some(deltas)
        );

        let deltas = vec![
            deltas::Delta {
                operations: vec![deltas::Operation::Insert((0, "hello".to_string()))],
                timestamp_ms: 0,
            },
            deltas::Delta {
                operations: vec![
                    deltas::Operation::Insert((5, "world".to_string())),
                    deltas::Operation::Insert((11, "!".to_string())),
                ],
                timestamp_ms: 1,
            },
        ];
        branch_writer.write_selected(Some(&vbranch0.id))?;
        deltas_writer.write(path, &deltas).unwrap();
        assert_eq!(deltas_reader.read_file(path).unwrap(), Some(deltas.clone()));
        assert_eq!(
            deltas_reader.read_virtual_file(&vbranch0.id, path).unwrap(),
            Some(vec![deltas::Delta {
                operations: vec![deltas::Operation::Insert((11, "!".to_string()))],
                timestamp_ms: 1,
            },])
        );
        assert_eq!(
            deltas_reader.read_virtual_file(&vbranch1.id, path).unwrap(),
            Some(vec![
                deltas[0].clone(),
                deltas::Delta {
                    operations: vec![deltas::Operation::Insert((5, "world".to_string()))],
                    timestamp_ms: 1,
                },
            ])
        );

        Ok(())
    }
}
