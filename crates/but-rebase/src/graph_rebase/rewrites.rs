use but_evo::Rewrite;

fn precs(rewrite: &Rewrite) -> Vec<gix::ObjectId> {
    match rewrite {
        Rewrite::Replace { prec, .. } | Rewrite::Split { prec, .. } | Rewrite::Prune { prec } => {
            vec![*prec]
        }
        Rewrite::Fold { precs, .. } => precs.clone(),
    }
}

fn succs(rewrite: &Rewrite) -> Vec<gix::ObjectId> {
    match rewrite {
        Rewrite::Replace { succ, .. } | Rewrite::Fold { succ, .. } => vec![*succ],
        Rewrite::Split { succs, .. } => succs.clone(),
        Rewrite::Prune { .. } => Vec::new(),
    }
}

fn precs_mut(rewrite: &mut Rewrite) -> &mut [gix::ObjectId] {
    match rewrite {
        Rewrite::Replace { prec, .. } | Rewrite::Split { prec, .. } | Rewrite::Prune { prec } => {
            std::slice::from_mut(prec)
        }
        Rewrite::Fold { precs, .. } => precs,
    }
}

fn succs_mut(rewrite: &mut Rewrite) -> &mut [gix::ObjectId] {
    match rewrite {
        Rewrite::Replace { succ, .. } | Rewrite::Fold { succ, .. } => std::slice::from_mut(succ),
        Rewrite::Split { succs, .. } => succs,
        Rewrite::Prune { .. } => &mut [],
    }
}

fn swap(ids: &mut [gix::ObjectId], from: gix::ObjectId, to: gix::ObjectId) {
    for id in ids {
        if *id == from {
            *id = to;
        }
    }
}

/// The rewrites of `log`, in order, with every one-to-one replacement folded into the
/// rewrite that made the commit it replaces or the rewrite that consumes its result.
// ponytail: finds a commit's maker by scanning back, quadratic in the log; index makers
// by successor if a log ever grows past a few thousand rewrites.
pub(crate) fn reduce(log: &[Rewrite]) -> Vec<Rewrite> {
    let mut reduced = Vec::<Rewrite>::new();
    for rewrite in log {
        match rewrite {
            Rewrite::Replace { prec, succ } if prec == succ => {}
            Rewrite::Replace { prec, succ } => {
                let Some(maker) = reduced
                    .iter_mut()
                    .rev()
                    .find(|maker| succs(maker).contains(prec))
                else {
                    reduced.push(rewrite.clone());
                    continue;
                };
                swap(succs_mut(maker), *prec, *succ);
                reduced.retain(
                    |made| !matches!(made, Rewrite::Replace { prec, succ } if prec == succ),
                );
            }
            Rewrite::Split { .. } | Rewrite::Fold { .. } | Rewrite::Prune { .. } => {
                let mut rewrite = rewrite.clone();
                for prec in precs(&rewrite) {
                    let Some(at) = reduced
                        .iter()
                        .rposition(|maker| succs(maker).contains(&prec))
                    else {
                        continue;
                    };
                    if let Some(Rewrite::Replace { prec: original, .. }) = reduced.get(at) {
                        swap(precs_mut(&mut rewrite), prec, *original);
                        reduced.remove(at);
                    }
                }
                reduced.push(rewrite);
            }
        }
    }
    reduced
}

#[cfg(test)]
mod tests {
    use but_evo::Rewrite;

    use super::reduce;

    fn id(hex: u8) -> gix::ObjectId {
        gix::ObjectId::from_hex(format!("{hex:040x}").as_bytes()).expect("valid hex")
    }

    fn replace(prec: u8, succ: u8) -> Rewrite {
        Rewrite::Replace {
            prec: id(prec),
            succ: id(succ),
        }
    }

    fn split(prec: u8, succs: [u8; 2]) -> Rewrite {
        Rewrite::Split {
            prec: id(prec),
            succs: succs.map(id).to_vec(),
        }
    }

    fn fold(precs: [u8; 2], succ: u8) -> Rewrite {
        Rewrite::Fold {
            precs: precs.map(id).to_vec(),
            succ: id(succ),
        }
    }

    #[test]
    fn splits_and_a_fold_keep_their_shape_as_replacements_fold_into_them() {
        let (a, a1, a2, b, b1, b2, c) = (0xa0, 0xa1, 0xa2, 0xb0, 0xb1, 0xb2, 0xc0);
        let log = [
            replace(a, 0xa9),
            split(0xa9, [a1, 0xa7]),
            replace(0xa7, a2),
            split(b, [0xb7, b2]),
            replace(0xb7, b1),
            fold([a2, b1], 0xc9),
            replace(0xc9, c),
        ];
        assert_eq!(
            reduce(&log),
            [split(a, [a1, a2]), split(b, [b1, b2]), fold([a2, b1], c)],
            "replacements before, between and after the splits and the fold vanish into them"
        );
    }

    #[test]
    fn replacements_chain_into_one_and_vanish_when_they_return() {
        assert_eq!(
            reduce(&[replace(1, 2), replace(2, 3)]),
            [replace(1, 3)],
            "a chain is one replacement"
        );
        assert_eq!(
            reduce(&[replace(1, 2), replace(2, 1)]),
            [],
            "a commit rewritten back to itself wasn't rewritten"
        );
    }

    #[test]
    fn a_fold_into_a_survivor_follows_its_later_replacement() {
        let (subject, target, squashed) = (1, 2, 3);
        assert_eq!(
            reduce(&[fold([subject, target], target), replace(target, squashed)]),
            [fold([subject, target], squashed)],
            "the target that absorbed the subject is itself replaced by the squashed commit"
        );
    }
}
