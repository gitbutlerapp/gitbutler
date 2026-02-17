import type { DependencyError, HunkDependencies } from "$lib/dependencies/dependencies";
import type { HunkAssignment, HunkAssignmentError } from "$lib/hunks/hunk";
import type { CoreUI } from "@gitbutler/core/api";

/** Contains the changes that are in the worktree */
export type WorktreeChanges = {
	/** Changes that could be committed. */
	readonly changes: TreeChange[];
	/**
	 * Changes that were in the index that we can't handle.
	 * The user can see them and interact with them to clear them out before a commit can be made.
	 */
	readonly ignoredChanges: IgnoredChange[];
	readonly assignments: HunkAssignment[];
	readonly assignmentsError: HunkAssignmentError | null;
	readonly dependencies: HunkDependencies | null;
	readonly dependenciesError: DependencyError | null;
};

/**
 * An entry in the worktree that changed and thus is eligible to being committed.
 * It either lives (or lived) in the in `.git/index`, or in the `worktree`.
 */
export type TreeChange = CoreUI.TreeChange;

export type TreeStats = {
	/** The total amount of lines added. */
	readonly linesAdded: number;
	/** The total amount of lines removed.*/
	readonly linesRemoved: number;
	/** The number of files added, removed or modified.*/
	readonly filesChanged: number;
};

/**
 * The list of changes and the stats
 */
export type TreeChanges = {
	/** The changes that were made to the tree. */
	readonly changes: TreeChange[];
	/** The stats of the changes. */
	readonly stats: TreeStats;
};

export function isTreeChange(something: unknown): something is TreeChange {
	return (
		typeof something === "object" &&
		something !== null &&
		"path" in something &&
		typeof something["path"] === "string" &&
		"pathBytes" in something &&
		Array.isArray(something["pathBytes"]) &&
		"status" in something &&
		isChangeStatus(something["status"])
	);
}

export type Flags = CoreUI.ModeFlags;

export type Status = CoreUI.TreeStatus;

export function isExecutableStatus(status: Status): boolean {
	switch (status.type) {
		case "Addition":
		case "Deletion":
			return false;
		case "Modification":
		case "Rename":
			return (
				status.subject.flags === "ExecutableBitAdded" ||
				status.subject.flags === "ExecutableBitRemoved"
			);
	}
}

export function isSubmoduleStatus(status: Status): boolean {
	switch (status.type) {
		case "Addition":
			return status.subject.state.kind === "Commit";
		case "Deletion":
			return status.subject.previousState.kind === "Commit";
		case "Modification":
			return (
				status.subject.state.kind === "Commit" || status.subject.previousState.kind === "Commit"
			);
		case "Rename":
			return (
				status.subject.state.kind === "Commit" || status.subject.previousState.kind === "Commit"
			);
	}
}

function isChangeStatus(something: unknown): something is Status {
	return (
		typeof something === "object" &&
		something !== null &&
		"type" in something &&
		typeof something["type"] === "string"
	);
}

/** A way to indicate that a path in the index isn't suitable for committing and needs to be dealt with.*/
export type IgnoredChange = {
	/** The worktree-relative path to the change.*/
	readonly path: string;
	/** The reason for the change being ignored.*/
	readonly status: IgnoredChangeStatus;
};

/** The status we can't handle.*/
type IgnoredChangeStatus =
	/** A conflicting entry in the index. The worktree state of the entry is unclear.*/
	| "Conflict"
	/** A change in the `.git/index` that was overruled by a change to the same path in the *worktree*.*/
	| "TreeIndex";
