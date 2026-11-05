use anyhow::{Context as _, Result};
use but_db::{DbHandle, ObsMarker};
use gix::bstr::BString;

use crate::{Marker, MarkerStore};

impl MarkerStore for DbHandle {
    fn find_by_prec(&self, prec: gix::ObjectId) -> Result<Vec<Marker>> {
        let rows = self.obs_markers().find_by_prec(prec.as_bytes())?;
        rows.into_iter().map(from_row).collect()
    }

    fn find_by_succ(&self, succ: gix::ObjectId) -> Result<Vec<Marker>> {
        let rows = self.obs_markers().find_by_succ(succ.as_bytes())?;
        rows.into_iter().map(from_row).collect()
    }

    fn write(&mut self, markers: &[Marker]) -> Result<()> {
        let rows = markers.iter().map(to_row).collect::<Result<Vec<_>>>()?;
        let mut transaction = self.immediate_transaction()?;
        transaction.obs_markers_mut().insert(&rows)?;
        transaction.commit()?;
        Ok(())
    }
}

fn to_row(marker: &Marker) -> Result<ObsMarker> {
    let id_bytes = |ids: &[gix::ObjectId]| ids.iter().map(|id| id.as_bytes().to_vec()).collect();
    let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
    hasher.update(&encode(marker));
    Ok(ObsMarker {
        id: hasher.try_finalize()?.as_bytes().to_vec(),
        prec: marker.prec.as_bytes().to_vec(),
        succs: id_bytes(&marker.succs),
        meta: marker
            .meta
            .iter()
            .map(|(key, value)| (key.to_vec(), value.to_vec()))
            .collect(),
        date_seconds: marker.date.seconds,
        date_offset: marker.date.offset,
        parents: marker.parents.as_deref().map(id_bytes),
    })
}

fn from_row(row: ObsMarker) -> Result<Marker> {
    Ok(Marker {
        prec: object_id(&row.prec)?,
        succs: object_ids(&row.succs)?,
        meta: row
            .meta
            .into_iter()
            .map(|(key, value)| (BString::from(key), BString::from(value)))
            .collect(),
        date: gix::date::Time::new(row.date_seconds, row.date_offset),
        parents: row.parents.as_deref().map(object_ids).transpose()?,
    })
}

fn object_ids(rows: &[Vec<u8>]) -> Result<Vec<gix::ObjectId>> {
    rows.iter().map(|row| object_id(row)).collect()
}

fn object_id(row: &[u8]) -> Result<gix::ObjectId> {
    gix::ObjectId::try_from(row).context("A stored marker holds an invalid object id")
}

fn encode(marker: &Marker) -> Vec<u8> {
    let mut out = Vec::new();
    put_bytes(&mut out, marker.prec.as_bytes());
    put_ids(&mut out, &marker.succs);
    put_len(&mut out, marker.meta.len());
    for (key, value) in &marker.meta {
        put_bytes(&mut out, key);
        put_bytes(&mut out, value);
    }
    out.extend(marker.date.seconds.to_be_bytes());
    out.extend(marker.date.offset.to_be_bytes());
    match &marker.parents {
        None => out.push(0),
        Some(parents) => {
            out.push(1);
            put_ids(&mut out, parents);
        }
    }
    out
}

fn put_ids(out: &mut Vec<u8>, ids: &[gix::ObjectId]) {
    put_len(out, ids.len());
    for id in ids {
        put_bytes(out, id.as_bytes());
    }
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    put_len(out, bytes.len());
    out.extend(bytes);
}

fn put_len(out: &mut Vec<u8>, len: usize) {
    out.extend((len as u64).to_be_bytes());
}
