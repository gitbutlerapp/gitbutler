import type { Commit, UpstreamCommit } from "@gitbutler/but-sdk";

/** Commit that is a part of a [`StackBranch`](gitbutler_stack::StackBranch) and, as such, containing state derived in relation to the specific branch.*/

/** Safely extract the creation time in milliseconds */
export function commitCreatedAt(commit: Commit | UpstreamCommit): number {
	return Number(commit.createdAt);
}

/** Safely extract the creation date from the commit */
export function commitCreatedAtDate(commit: Commit | UpstreamCommit): Date {
	return new Date(commitCreatedAt(commit));
}

/** If the commit is in `LocalAndRemote` state, extract the subject (the remote commit ID) */
export function commitStateSubject(commit: Commit): string | null {
	switch (commit.state.type) {
		case "LocalOnly":
			return null;
		case "Integrated":
			return null;
		case "LocalAndRemote":
			return commit.state.subject;
	}
}

/**
 * Commit that is only at the remote.
 * Unlike the `Commit` struct, there is no knowledge of GitButler concepts like conflicted state etc.
 */

export function isCommit(something: Commit | UpstreamCommit): something is Commit {
	return "state" in something;
}

export function extractUpstreamCommitId(commit: Commit | UpstreamCommit): string | undefined {
	if (isCommit(commit)) {
		if (commit.state.type === "LocalAndRemote") {
			return commit.state.subject;
		}
	}
	return undefined;
}

/** Represents the author of a commit. */

/** Represents the state a commit could be in. */
