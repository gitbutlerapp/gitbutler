mod get_gerrit_flags {
    use crate::push::{Args, get_gerrit_flags};

    #[test]
    fn non_gerrit_mode() {
        let args = Args {
            branch_id: "test".to_string(),
            with_force: true,
            skip_force_push_protection: false,
            run_hooks: true,
            wip: false,
            ready: false,
            hashtag: vec![],
            topic: None,
            topic_from_branch: false,
            private: false,
        };

        let result = get_gerrit_flags(&args, "test-branch", false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![]);
    }

    #[test]
    fn error_when_flags_without_gerrit_mode() {
        let args = Args {
            branch_id: "test".to_string(),
            with_force: true,
            skip_force_push_protection: false,
            run_hooks: true,
            wip: true,
            ready: false,
            hashtag: vec![],
            topic: None,
            topic_from_branch: false,
            private: false,
        };

        let result = get_gerrit_flags(&args, "test-branch", false);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("can only be used when gerrit_mode is enabled")
        );
    }

    #[test]
    fn default_ready() {
        let args = Args {
            branch_id: "test".to_string(),
            with_force: true,
            skip_force_push_protection: false,
            run_hooks: true,
            wip: false,
            ready: false,
            hashtag: vec![],
            topic: None,
            topic_from_branch: false,
            private: false,
        };

        let result = get_gerrit_flags(&args, "test-branch", true);
        assert!(result.is_ok());
        let flags = result.unwrap();
        assert_eq!(flags.len(), 1);
        assert!(matches!(flags[0], but_gerrit::PushFlag::Ready));
    }

    #[test]
    fn wip() {
        let args = Args {
            branch_id: "test".to_string(),
            with_force: true,
            skip_force_push_protection: false,
            run_hooks: true,
            wip: true,
            ready: false,
            hashtag: vec![],
            topic: None,
            topic_from_branch: false,
            private: false,
        };

        let result = get_gerrit_flags(&args, "test-branch", true);
        assert!(result.is_ok());
        let flags = result.unwrap();
        assert_eq!(flags.len(), 1);
        assert!(matches!(flags[0], but_gerrit::PushFlag::Wip));
    }

    #[test]
    fn multiple_hashtags() {
        let args = Args {
            branch_id: "test".to_string(),
            with_force: true,
            skip_force_push_protection: false,
            run_hooks: true,
            wip: false,
            ready: false,
            hashtag: vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()],
            topic: None,
            topic_from_branch: false,
            private: false,
        };

        let result = get_gerrit_flags(&args, "test-branch", true);
        assert!(result.is_ok());
        let flags = result.unwrap();
        assert_eq!(flags.len(), 4); // Ready + 3 hashtags

        let ready_count = flags
            .iter()
            .filter(|f| matches!(f, but_gerrit::PushFlag::Ready))
            .count();
        assert_eq!(ready_count, 1);

        let hashtag_count = flags
            .iter()
            .filter(|f| matches!(f, but_gerrit::PushFlag::Hashtag(_)))
            .count();
        assert_eq!(hashtag_count, 3);
    }

    #[test]
    fn topic_from_custom() {
        let args = Args {
            branch_id: "test".to_string(),
            with_force: true,
            skip_force_push_protection: false,
            run_hooks: true,
            wip: false,
            ready: false,
            hashtag: vec![],
            topic: Some("custom-topic".to_string()),
            topic_from_branch: false,
            private: false,
        };

        let result = get_gerrit_flags(&args, "test-branch", true);
        assert!(result.is_ok());
        let flags = result.unwrap();
        assert_eq!(flags.len(), 2); // Ready + Topic

        let topic_flags: Vec<_> = flags
            .iter()
            .filter_map(|f| match f {
                but_gerrit::PushFlag::Topic(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(topic_flags, vec!["custom-topic"]);
    }

    #[test]
    fn topic_from_branch() {
        let args = Args {
            branch_id: "test".to_string(),
            with_force: true,
            skip_force_push_protection: false,
            run_hooks: true,
            wip: false,
            ready: false,
            hashtag: vec![],
            topic: None,
            topic_from_branch: true,
            private: false,
        };

        let result = get_gerrit_flags(&args, "my-branch-name", true);
        assert!(result.is_ok());
        let flags = result.unwrap();
        assert_eq!(flags.len(), 2); // Ready + Topic

        let topic_flags: Vec<_> = flags
            .iter()
            .filter_map(|f| match f {
                but_gerrit::PushFlag::Topic(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(topic_flags, vec!["my-branch-name"]);
    }

    #[test]
    fn private() {
        let args = Args {
            branch_id: "test".to_string(),
            with_force: true,
            skip_force_push_protection: false,
            run_hooks: true,
            wip: false,
            ready: false,
            hashtag: vec![],
            topic: None,
            topic_from_branch: false,
            private: true,
        };

        let result = get_gerrit_flags(&args, "test-branch", true);
        assert!(result.is_ok());
        let flags = result.unwrap();
        assert_eq!(flags.len(), 2); // Ready + Private

        let private_count = flags
            .iter()
            .filter(|f| matches!(f, but_gerrit::PushFlag::Private))
            .count();
        assert_eq!(private_count, 1);
    }

    #[test]
    fn all_combined() {
        let args = Args {
            branch_id: "test".to_string(),
            with_force: true,
            skip_force_push_protection: false,
            run_hooks: true,
            wip: true,
            ready: false,
            hashtag: vec!["tag1".to_string(), "tag2".to_string()],
            topic: Some("custom-topic".to_string()),
            topic_from_branch: false,
            private: true,
        };

        let result = get_gerrit_flags(&args, "test-branch", true);
        assert!(result.is_ok());
        let flags = result.unwrap();
        assert_eq!(flags.len(), 5); // Wip + 2 hashtags + Topic + Private

        let wip_count = flags
            .iter()
            .filter(|f| matches!(f, but_gerrit::PushFlag::Wip))
            .count();
        assert_eq!(wip_count, 1);

        let hashtag_count = flags
            .iter()
            .filter(|f| matches!(f, but_gerrit::PushFlag::Hashtag(_)))
            .count();
        assert_eq!(hashtag_count, 2);

        let topic_count = flags
            .iter()
            .filter(|f| matches!(f, but_gerrit::PushFlag::Topic(_)))
            .count();
        assert_eq!(topic_count, 1);

        let private_count = flags
            .iter()
            .filter(|f| matches!(f, but_gerrit::PushFlag::Private))
            .count();
        assert_eq!(private_count, 1);
    }

    #[test]
    fn empty_hashtag_error() {
        let args = Args {
            branch_id: "test".to_string(),
            with_force: true,
            skip_force_push_protection: false,
            run_hooks: true,
            wip: false,
            ready: false,
            hashtag: vec!["  ".to_string()],
            topic: None,
            topic_from_branch: false,
            private: false,
        };

        let result = get_gerrit_flags(&args, "test-branch", true);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Hashtag cannot be empty")
        );
    }

    #[test]
    fn empty_topic_error() {
        let args = Args {
            branch_id: "test".to_string(),
            with_force: true,
            skip_force_push_protection: false,
            run_hooks: true,
            wip: false,
            ready: false,
            hashtag: vec![],
            topic: Some("  ".to_string()),
            topic_from_branch: false,
            private: false,
        };

        let result = get_gerrit_flags(&args, "test-branch", true);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Topic cannot be empty")
        );
    }
}
