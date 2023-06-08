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
    pub fn new(repository: &'writer gb_repository::Repository) -> Result<Self> {
        let writer = writer::DirWriter::open(repository.root());
        Ok(Self { writer, repository })
    }

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

        for (vbid, deltas) in self.split_virtual_branches(path, deltas)? {
            let raw_deltas = serde_json::to_string(&deltas)?;
            self.writer.write_string(
                &format!("branches/{}/deltas/{}", vbid, path.display()),
                &raw_deltas,
            )?;
            log::info!(
                "{}: wrote deltas for virtual branch {} for {}",
                self.repository.project_id,
                vbid,
                path.display()
            );
        }

        Ok(())
    }

    // returns a map of virtual branch id -> deltas to append
    fn split_virtual_branches<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        deltas: &Vec<Delta>,
    ) -> Result<HashMap<String, Vec<Delta>>> {
        let dir_reader = reader::DirReader::open(self.repository.root());

        // first, read all virtual branches
        let mut virtual_branches = virtual_branches::Iterator::new(&dir_reader)?
            .collect::<Result<Vec<branch::Branch>, crate::reader::Error>>()?
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

                for vb_delta in vb_deltas {
                    let taken_remaining = vb_delta.take(delta);
                    // if delta was taken by an existing virtual delta, add it to the result
                    if let Some(taken) = taken_remaining.0 {
                        let new_deltas = new_deltas_by_vbranch
                            .entry(vbranch.id.clone())
                            .or_insert_with(Vec::new);
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
