use std::fs;

/// These tests cover the `sign_if_configured` property on the Step::Pick.
use anyhow::Result;
use but_graph::Graph;
use but_rebase::graph_rebase::{GraphExt, Pick, Step};
use but_testsupport::{cat_commit, visualize_commit_graph_all};

use crate::utils::{fixture_writable_with_signing, standard_options};

#[test]
fn assert_consistent_private_key() -> Result<()> {
    let (_repo, tmpdir, _meta) = fixture_writable_with_signing("four-commits-signed")?;

    let key = fs::read_to_string(tmpdir.path().join("signature.key"))?;
    insta::assert_snapshot!(key, @r"
    -----BEGIN OPENSSH PRIVATE KEY-----
    b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAABFwAAAAdzc2gtcn
    NhAAAAAwEAAQAAAQEAuBhnTC0+8nJnjSpZEh7wBsBiEpiC3RtZfdnXo/JmNYQX4UXH1tFJ
    OFjQFzjlM3OifXff9ppNYwGc71EM/DnTBkfZQsjEXxD3QGQGr0YjiVyWLPyi+nCfd7M3pN
    C75RvUttNYPYY5oLJQqm5Af3oCyY5Pko0BJ9t0mN/x7Ns76RmDz4nUcxLzeA7GHGPXkbB/
    VwIkAidev+mFhfwGYBlZIdke7x+jLogbWDV262vZDIAYV13AMo5uytt6Ow6HBsXu7s9MQZ
    ZY7rdmUpLn9B9eDiEKjJaytNbuVWojpeDGTjM5pT4Ses1KvYEFcZJKACp7W+jxNVaCA2H8
    AJ2dlrhjoQAAA8hDQKQaQ0CkGgAAAAdzc2gtcnNhAAABAQC4GGdMLT7ycmeNKlkSHvAGwG
    ISmILdG1l92dej8mY1hBfhRcfW0Uk4WNAXOOUzc6J9d9/2mk1jAZzvUQz8OdMGR9lCyMRf
    EPdAZAavRiOJXJYs/KL6cJ93szek0LvlG9S201g9hjmgslCqbkB/egLJjk+SjQEn23SY3/
    Hs2zvpGYPPidRzEvN4DsYcY9eRsH9XAiQCJ16/6YWF/AZgGVkh2R7vH6MuiBtYNXbra9kM
    gBhXXcAyjm7K23o7DocGxe7uz0xBlljut2ZSkuf0H14OIQqMlrK01u5VaiOl4MZOMzmlPh
    J6zUq9gQVxkkoAKntb6PE1VoIDYfwAnZ2WuGOhAAAAAwEAAQAAAQBzUx5K00FOoiqKfU/l
    ESpuIFCPs6ivGHX8Z941nyE2PzSyc4NX6C2FNeXN1l+G1tag4NqVYl4+OoF0TgLjctnmYl
    YRBzI1F6y8Uqz5WefjIfQV5IG4f5r2YnfmMLi0MrYTfdwWVqJ9L5dm3MBc2zMpzpO8i8aA
    kHK/XfLw3Pnv8HLgbfmxRDVfMJ46UtsMuTtHcFQdXpQh9JpOlbG+xvCKfCSN+W/SoaSGQo
    1Bt96/MSPPausBnSkcyk4LaeHDO3h2TjVfxCd6fTN0JqgMQ4vvHkiz7UPhx6T0ofkDm+gc
    hbZ8RDOY7msYQcdYziwXRozkWmc/u3fhw37Orji6SzgBAAAAgBurWQGzpqnHSTDbvWOEkF
    LLW3m87GY6MwZFbGnDR2T5sH5nLsVsAgV7D2JwAigM5lGf245E5zyOUSo5QGaVg67mu4Fd
    j05zDi7FESnADqZPCwyH4UrU0jFTTsbgWlo++uEH9ghlYkOodoCBeiG7t7+B1j9dyBWMVJ
    XsV1VmYJSLAAAAgQDc6HENFCofL+9ZI02ATx0z9I4yfEE8f4l4azGVa18ziRFsuH//vzOO
    ZNKUcHmnD5qWSOWzl7UMHfcn2cdv75Oac2CJEAg/lIEtPcTwDngHiESZtqiwOcInwxH1iN
    d4trHNnyvtFoaPWJR0RQ5gkOQrPMd/ZqXpTugkS2pjqNcNwQAAAIEA1Vbra7Tys8xfUZFz
    vZtHxp6cDZ9MV/YH0RLvGqjPueAPerqUgMVnGa/6yRABfPauLhqfqs2q8eMjcfb5hnZ8lB
    YGsxf0dDAMkeeAsKmtMroNGqDHODfnBVyemBH+YuvBR7IS64zOpEGU9DpeDnoqBXOezmkW
    +VXuLOvsScuijeEAAAAQdGVzdEBleGFtcGxlLmNvbQECAw==
    -----END OPENSSH PRIVATE KEY-----
    ");

    Ok(())
}

#[test]
fn commits_maintain_state_if_not_cherry_picked() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable_with_signing("four-commits-signed")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @r"
    * dd72792 (HEAD -> main, c) c
    * e5aa7b5 (b) b
    * 3bfeb52 (a) a
    * b6e2f57 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut editor = graph.to_editor(&repo)?;

    // Modify the "c" commit to no longer be signed
    let c = repo.rev_parse_single("c")?;
    let c_sel = editor.select_commit(c.detach())?;
    let mut pick = Pick::new_pick(c.detach());
    pick.sign_if_configured = false;
    editor.replace(c_sel, Step::Pick(pick))?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    assert_eq!(visualize_commit_graph_all(&repo)?, before);

    Ok(())
}

#[test]
fn commits_are_signed_by_default() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable_with_signing("four-commits-signed")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @r"
    * dd72792 (HEAD -> main, c) c
    * e5aa7b5 (b) b
    * 3bfeb52 (a) a
    * b6e2f57 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut editor = graph.to_editor(&repo)?;

    // Remove the "b" commit so "c" gets cherry-picked
    let b = repo.rev_parse_single("b")?;
    let b_sel = editor.select_commit(b.detach())?;
    editor.replace(b_sel, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 0e3a49e (HEAD -> main, c) c
    * 3bfeb52 (b, a) a
    * b6e2f57 (base) base
    ");

    insta::assert_snapshot!(cat_commit(&repo, "c")?, @r"
    tree ea0372ea78d32151cb4c2b6a05a084817947c8f3
    parent 3bfeb524461f65f82bf5027fc895fe9fd5f36203
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946771200 +0000
    gitbutler-headers-version 2
    gitbutler-change-id 00000000-0000-0000-0000-000000000001
    gpgsig -----BEGIN SSH SIGNATURE-----
     U1NIU0lHAAAAAQAAARcAAAAHc3NoLXJzYQAAAAMBAAEAAAEBALgYZ0wtPvJyZ40qWRIe8A
     bAYhKYgt0bWX3Z16PyZjWEF+FFx9bRSThY0Bc45TNzon133/aaTWMBnO9RDPw50wZH2ULI
     xF8Q90BkBq9GI4lcliz8ovpwn3ezN6TQu+Ub1LbTWD2GOaCyUKpuQH96AsmOT5KNASfbdJ
     jf8ezbO+kZg8+J1HMS83gOxhxj15Gwf1cCJAInXr/phYX8BmAZWSHZHu8foy6IG1g1dutr
     2QyAGFddwDKObsrbejsOhwbF7u7PTEGWWO63ZlKS5/QfXg4hCoyWsrTW7lVqI6Xgxk4zOa
     U+EnrNSr2BBXGSSgAqe1vo8TVWggNh/ACdnZa4Y6EAAAADZ2l0AAAAAAAAAAZzaGE1MTIA
     AAEUAAAADHJzYS1zaGEyLTUxMgAAAQBQttFHnDf0WFY4pmCG2X1gE2An415/DQstH7g2Ei
     V4cLghc6+50sngUNt75JtBexJ8T6c/EOR7Yw//lFdex2D1M0kihaLEj63tS4IFWsM3spvp
     5gVHrGKFF92Kq5GnaJKACOBFAjskrrOvWYiOATQfcOov085GMAeSDKwA2vx29NVSdepYFk
     p12AMVyusxFkl+TTqm7VIqcrb43t84PqnX6L7U42Pmuzh9gR84Roz3sQgoMNlC37wWUoGg
     uBEmtyPHLL/5AMKE/GGNmDZgiofsKfdyrRWEBmWbBcPt293hIQm4zDJghwB7H8w5fU3Buz
     DtgrahrnNvZB2Y4QeGVoTc
     -----END SSH SIGNATURE-----

    c
    ");

    Ok(())
}

#[test]
fn when_cherry_picking_dont_resign_if_not_set() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable_with_signing("four-commits-signed")?;

    let before = visualize_commit_graph_all(&repo)?;
    insta::assert_snapshot!(before, @r"
    * dd72792 (HEAD -> main, c) c
    * e5aa7b5 (b) b
    * 3bfeb52 (a) a
    * b6e2f57 (base) base
    ");

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let mut editor = graph.to_editor(&repo)?;

    // Modify the "c" commit to no longer be signed
    let c = repo.rev_parse_single("c")?;
    let c_sel = editor.select_commit(c.detach())?;
    let mut pick = Pick::new_pick(c.detach());
    pick.sign_if_configured = false;
    editor.replace(c_sel, Step::Pick(pick))?;

    // Remove the "b" commit so "c" gets cherry-picked
    let b = repo.rev_parse_single("b")?;
    let b_sel = editor.select_commit(b.detach())?;
    editor.replace(b_sel, Step::None)?;

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 38e3281 (HEAD -> main, c) c
    * 3bfeb52 (b, a) a
    * b6e2f57 (base) base
    ");

    insta::assert_snapshot!(cat_commit(&repo, "c")?, @r"
    tree ea0372ea78d32151cb4c2b6a05a084817947c8f3
    parent 3bfeb524461f65f82bf5027fc895fe9fd5f36203
    author author <author@example.com> 946684800 +0000
    committer Committer (Memory Override) <committer@example.com> 946771200 +0000
    gitbutler-headers-version 2
    gitbutler-change-id 00000000-0000-0000-0000-000000000001

    c
    ");

    Ok(())
}
