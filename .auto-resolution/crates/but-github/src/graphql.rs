pub const GQL_SET_PR_READY_FOR_REVIEW: &str = r#"
    mutation MarkPullRequestReadyForReview($pullRequestId: ID!) {
      markPullRequestReadyForReview(input: { pullRequestId: $pullRequestId }) {
        pullRequest {
          id
        }
      }
    }
    "#;

pub const GQL_SET_PR_DRAFT: &str = r#"
    mutation ConvertPullRequestToDraft($pullRequestId: ID!) {
      convertPullRequestToDraft(input: { pullRequestId: $pullRequestId }) {
        pullRequest {
          id
        }
      }
    }
    "#;

pub const GQL_ENABLE_PR_AUTO_MERGE: &str = r#"
    mutation EnablePullRequestAutoMerge($input: EnablePullRequestAutoMergeInput!) {
      enablePullRequestAutoMerge(input: $input) {
        pullRequest {
          id
        }
      }
    }
    "#;

pub const GQL_DISABLE_PR_AUTO_MERGE: &str = r#"
    mutation DisablePullRequestAutoMerge($pullRequestId: ID!) {
      disablePullRequestAutoMerge(input: { pullRequestId: $pullRequestId }) {
        pullRequest {
          id
        }
      }
    }
    "#;

pub const GQL_GET_PR_NODE_ID: &str = r#"
    query PullRequestNodeId($owner: String!, $repo: String!, $number: Int!) {
      repository(owner: $owner, name: $repo) {
        pullRequest(number: $number) {
          id
        }
      }
    }
    "#;
