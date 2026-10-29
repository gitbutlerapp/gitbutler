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

/// Commits resolve their author to a GitHub account (`author.user`), which
/// REST's timeline never reports — without a login, "your own push" cannot
/// be told apart from anyone else's.
/// The single-review fetch. GraphQL is the one source that lists a bot
/// among a pull request's requested reviewers; REST leaves them out.
pub const GQL_GET_PR: &str = r#"
    query PullRequest($owner: String!, $repo: String!, $number: Int!) {
      repository(owner: $owner, name: $repo) {
        pullRequest(number: $number) {
          url
          number
          title
          body
          isDraft
          headRefName
          baseRefName
          headRefOid
          createdAt
          updatedAt
          mergedAt
          closedAt
          mergeCommit {
            oid
          }
          author {
            __typename
            login
            avatarUrl
            ... on User {
              databaseId
              name
            }
            ... on Bot {
              databaseId
            }
          }
          labels(first: 100) {
            nodes {
              name
              color
              description
            }
          }
          headRepository {
            sshUrl
            url
            isFork
            owner {
              login
            }
          }
          reviewRequests(first: 100) {
            nodes {
              requestedReviewer {
                __typename
                ... on User {
                  databaseId
                  login
                  avatarUrl
                  name
                }
                ... on Bot {
                  databaseId
                  login
                  avatarUrl
                }
              }
            }
          }
          autoMergeRequest {
            enabledAt
          }
        }
      }
    }
    "#;

/// Diff-anchored review threads, which REST does not expose: `/pulls/{n}/comments`
/// returns a flat comment list with neither thread grouping nor resolution state.
pub const GQL_LIST_PR_REVIEW_THREADS: &str = r#"
    query PullRequestReviewThreads($owner: String!, $repo: String!, $number: Int!, $cursor: String) {
      repository(owner: $owner, name: $repo) {
        pullRequest(number: $number) {
          reviewThreads(first: 100, after: $cursor) {
            pageInfo {
              hasNextPage
              endCursor
            }
            nodes {
              id
              isResolved
              isOutdated
              path
              line
              startLine
              originalLine
              diffSide
              comments(first: 100) {
                pageInfo {
                  hasNextPage
                }
                nodes {
                  databaseId
                  body
                  createdAt
                  lastEditedAt
                  url
                  diffHunk
                  pullRequestReview {
                    databaseId
                  }
                  author {
                    __typename
                    login
                    avatarUrl
                    ... on User {
                      databaseId
                      name
                    }
                    ... on Bot {
                      databaseId
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
    "#;

/// Submitted reviews with their reactions. GraphQL rather than REST: a
/// review is only reactable through GraphQL, and `/pulls/{n}/reviews`
/// reports neither the reactions nor the node id the reaction mutations
/// address.
pub const GQL_LIST_PR_REVIEWS: &str = r#"
    query PullRequestReviews($owner: String!, $repo: String!, $number: Int!, $cursor: String) {
      repository(owner: $owner, name: $repo) {
        pullRequest(number: $number) {
          reviews(first: 100, after: $cursor) {
            pageInfo {
              hasNextPage
              endCursor
            }
            nodes {
              id
              databaseId
              state
              body
              submittedAt
              url
              author {
                __typename
                login
                avatarUrl
                ... on User {
                  databaseId
                  name
                }
                ... on Bot {
                  databaseId
                }
              }
              reactions(first: 100) {
                pageInfo {
                  hasNextPage
                }
                nodes {
                  databaseId
                  content
                  user {
                    __typename
                    login
                    avatarUrl
                    databaseId
                    name
                  }
                }
              }
            }
          }
        }
      }
    }
    "#;
