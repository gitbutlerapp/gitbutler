mod create_reference {
    use but_api::commands::stack::create_reference;
    use but_api::commands::stack::create_reference::{Params, Request};
    use but_api::hex_hash::HexHash;
    use but_workspace::branch::ReferencePosition;
    use gitbutler_project::ProjectId;
    use std::str::FromStr;

    #[test]
    fn params_to_json() {
        let params_no_new_stack = Params {
            project_id: ProjectId::from_number_for_testing(1),
            request: Request {
                new_name: "new-branch".to_string(),
                anchor: None,
            },
        };
        insta::assert_json_snapshot!(params_no_new_stack, @r#"
        {
          "projectId": "00000000-0000-0000-0000-000000000001",
          "request": {
            "newName": "new-branch",
            "anchor": null
          }
        }
        "#);

        let params_dependent_branch_at_commit = Params {
            project_id: ProjectId::from_number_for_testing(1),
            request: Request {
                new_name: "new-branch".to_string(),
                anchor: Some(create_reference::Anchor::AtCommit {
                    commit_id: HexHash(
                        gix::ObjectId::from_str("5c69907b1244089142905dba380371728e2e8160")
                            .unwrap(),
                    ),
                    position: ReferencePosition::Above,
                }),
            },
        };
        insta::assert_json_snapshot!(params_dependent_branch_at_commit, @r#"
        {
          "projectId": "00000000-0000-0000-0000-000000000001",
          "request": {
            "newName": "new-branch",
            "anchor": {
              "type": "atCommit",
              "subject": {
                "commit_id": "5c69907b1244089142905dba380371728e2e8160",
                "position": "Above"
              }
            }
          }
        }
        "#);

        let params_dependent_branch_at_commit = Params {
            project_id: ProjectId::from_number_for_testing(1),
            request: Request {
                new_name: "new-branch".to_string(),
                anchor: Some(create_reference::Anchor::AtReference {
                    short_name: "anchor-ref".into(),
                    position: ReferencePosition::Above,
                }),
            },
        };
        insta::assert_json_snapshot!(params_dependent_branch_at_commit, @r#"
        {
          "projectId": "00000000-0000-0000-0000-000000000001",
          "request": {
            "newName": "new-branch",
            "anchor": {
              "type": "atReference",
              "subject": {
                "short_name": "anchor-ref",
                "position": "Above"
              }
            }
          }
        }
        "#);
    }
}
