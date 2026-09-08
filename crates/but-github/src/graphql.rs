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
pub const GQL_LIST_PR_TIMELINE: &str = r#"
    query PullRequestTimeline($owner: String!, $repo: String!, $number: Int!, $cursor: String) {
      repository(owner: $owner, name: $repo) {
        pullRequest(number: $number) {
          timelineItems(first: 100, after: $cursor, itemTypes: [PULL_REQUEST_COMMIT, REVIEW_REQUESTED_EVENT]) {
            pageInfo {
              hasNextPage
              endCursor
            }
            nodes {
              __typename
              ... on PullRequestCommit {
                commit {
                  oid
                  messageHeadline
                  author {
                    name
                    date
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
              ... on ReviewRequestedEvent {
                createdAt
                actor {
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

/// Reply into an existing review thread. Keyed on the thread's node id, which
/// is what the reader already carries; REST's reply endpoint wants the id of a
/// comment inside the thread instead.
///
/// The comment selection must stay in step with `GQL_LIST_PR_REVIEW_THREADS`,
/// since both decode into the same type.
pub const GQL_ADD_REVIEW_THREAD_REPLY: &str = r#"
    mutation AddPullRequestReviewThreadReply($threadId: ID!, $body: String!) {
      addPullRequestReviewThreadReply(
        input: { pullRequestReviewThreadId: $threadId, body: $body }
      ) {
        comment {
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

/// The reaction selection must stay in step with `GQL_LIST_PR_REVIEWS`,
/// since both decode into the same type.
pub const GQL_ADD_REACTION: &str = r#"
    mutation AddReaction($subjectId: ID!, $content: ReactionContent!) {
      addReaction(input: { subjectId: $subjectId, content: $content }) {
        reaction {
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
    "#;

/// GraphQL removes by kind rather than by reaction id: a viewer can only
/// hold one reaction of each kind on a subject, so the pair is enough.
pub const GQL_REMOVE_REACTION: &str = r#"
    mutation RemoveReaction($subjectId: ID!, $content: ReactionContent!) {
      removeReaction(input: { subjectId: $subjectId, content: $content }) {
        reaction {
          databaseId
        }
      }
    }
    "#;
