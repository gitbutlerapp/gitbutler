// Class transformers will bust a gut if this isn't imported first
import 'reflect-metadata';

import { msSinceDaysAgo } from '$lib/utils/time';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { ForgeUser, PullRequest } from '$lib/forge/interface/types';

export type GroupedSidebarEntries = Record<
	'applied' | 'authored' | 'review' | 'today' | 'yesterday' | 'lastWeek' | 'older',
	SidebarEntrySubject[]
>;

/**
 * Represents a branch that exists for the repository
 * This also combines the concept of a remote, local and virtual branch in order to provide a unified interface for the UI
 * Branch entry is not meant to contain all of the data a branch can have (e.g. full commit history, all files and diffs, etc.).
 * It is intended a summary that can be quickly retrieved and displayed in the UI.
 * For more detailed information, each branch can be queried individually for it's `BranchData`.
 */
export type BranchListing = {
	/** The name of the branch (e.g. `main`, `feature/branch`), excluding the remote name */
	name: string;
	/**
	 * This is a list of remote that this branch can be found on (e.g. `origin`, `upstream` etc.).
	 * If this branch is a local branch, this list will be empty.
	 */
	remotes: string[];
	/** The branch may or may not have a virtual branch associated with it */
	stack?: StackReference | undefined;
	/**
	 * Timestamp in milliseconds since the branch was last updated.
	 * This includes any commits, uncommited changes or even updates to the branch metadata (e.g. renaming).
	 */
	updatedAt: string;
	/** The person who commited the head commit */
	lastCommiter: Author;
	/** Whether or not there is a local branch as part of the grouping */
	hasLocal: boolean;
};

/** Represents a reference to an associated virtual branch */
export type StackReference = {
	/** A non-normalized name of the branch, set by the user */
	givenName: string;
	/** Virtual Branch UUID identifier */
	id: string;
	/** Determines if the virtual branch is applied in the workspace */
	inWorkspace: boolean;
	/**
   List of branch names that are part of the stack
   Ordered from newest to oldest (the most recent branch is first in the list)
    */
	branches: string[];
	/** Pull Request numbes by branch name associated with the stack */
	pullRequests: Record<string, number>;
};

/** Represents a "commit author" or "signature", based on the data from ther git history */
export type Author = {
	/** The name of the author as configured in the git config */
	name?: string | undefined;
	/** The email of the author as configured in the git config */
	email?: string | undefined;
	/** The gravatar id of the author */
	gravatarUrl?: string | undefined;
};

/** Represents a fat struct with all the data associated with a branch */
export class BranchListingDetails {
	/** The name of the branch (e.g. `main`, `feature/branch`), excluding the remote name */
	name!: string;
	/**
	 * The number of lines added within the branch
	 * Since the virtual branch, local branch and the remote one can have different number of lines removed,
	 * the value from the virtual branch (if present) takes the highest precedence,
	 * followed by the local branch and then the remote branches (taking the max if there are multiple).
	 * If this branch has a virutal branch, lines_added does NOT include the uncommitted lines.
	 */
	linesAdded!: number;
	/**
	 * The number of lines removed within the branch
	 * Since the virtual branch, local branch and the remote one can have different number of lines removed,
	 * the value from the virtual branch (if present) takes the highest precedence,
	 * followed by the local branch and then the remote branches (taking the max if there are multiple)
	 * If this branch has a virutal branch, lines_removed does NOT include the uncommitted lines.
	 */
	linesRemoved!: number;
	/**
	 * The number of files that were modified within the branch
	 * Since the virtual branch, local branch and the remote one can have different number files modified,
	 * the value from the virtual branch (if present) takes the highest precedence,
	 * followed by the local branch and then the remote branches (taking the max if there are multiple)
	 */
	numberOfFiles!: number;
	/**
	 * The number of commits associated with a branch
	 * Since the virtual branch, local branch and the remote one can have different number of commits,
	 * the value from the virtual branch (if present) takes the highest precedence,
	 * followed by the local branch and then the remote branches (taking the max if there are multiple)
	 */
	numberOfCommits!: number;
	/**
	 * A list of authors that have contributes commits to this branch.
	 * In the case of multiple remote tracking branches, it takes the full list of unique authors.
	 */
	authors!: Author[];
	/** The branch may or may not have a virtual branch associated with it */
	stack?: StackReference | undefined;
}

type PullRequestEntrySubject = {
	type: 'pullRequest';
	subject: PullRequest;
	branchListing?: BranchListing;
};

type BranchListingEntrySubject = {
	type: 'branchListing';
	subject: BranchListing;
	prs: PullRequest[];
};

export type SidebarEntrySubject = PullRequestEntrySubject | BranchListingEntrySubject;

export const BRANCH_FILTER_OPTIONS = ['all', 'pullRequest', 'local'] as const;
export type BranchFilterOption = (typeof BRANCH_FILTER_OPTIONS)[number];

export function isBranchFilterOption(something: unknown): something is BranchFilterOption {
	return (
		typeof something === 'string' && BRANCH_FILTER_OPTIONS.includes(something as BranchFilterOption)
	);
}

function getEntryUpdatedDate(entry: SidebarEntrySubject) {
	return new Date(
		entry.type === 'branchListing' ? entry.subject.updatedAt : entry.subject.modifiedAt
	);
}

function getEntryName(entry: SidebarEntrySubject) {
	return entry.type === 'branchListing' ? entry.subject.name : entry.subject.title;
}

function getEntryWorkspaceStatus(entry: SidebarEntrySubject) {
	return entry.type === 'branchListing' ? entry.subject.stack?.inWorkspace : undefined;
}

function isReviewerOfEntry(userId: number | undefined, entry: SidebarEntrySubject): boolean {
	if (userId === undefined) return false;
	if (entry.type === 'pullRequest') {
		return entry.subject.reviewers.some((r) => r.id === userId);
	}
	return entry.prs.some((pr) => pr.reviewers.some((r) => r.id === userId));
}

function isAuthoreOfEntry(userId: number | undefined, entry: SidebarEntrySubject): boolean {
	if (userId === undefined) return false;
	if (entry.type === 'pullRequest') {
		return entry.subject.author?.id === userId;
	}
	return entry.prs.some((pr) => pr.author?.id === userId);
}

export function combineBranchesAndPrs(
	pullRequests: PullRequest[],
	branchList: BranchListing[],
	selectedOption: BranchFilterOption
) {
	const prMap = Object.fromEntries(pullRequests.map((pr) => [pr.sourceBranch, pr]));

	const listingSubjects: BranchListingEntrySubject[] = branchList.map((subject) => ({
		type: 'branchListing',
		subject: subject,
		prs:
			getBranchNames(subject)
				.map((name) => prMap[name])
				.filter(isDefined) || []
	}));

	const attachedPrs = new Set(listingSubjects.flatMap((item) => item.prs.map((pr) => pr.number)));
	const prs: PullRequestEntrySubject[] = pullRequests
		.filter((pr) => !attachedPrs.has(pr.number))
		.map((pullRequests) => ({ type: 'pullRequest', subject: pullRequests }));

	const result = [...prs, ...listingSubjects];

	result.sort((a, b) => {
		const timeDifference = getEntryUpdatedDate(b).getTime() - getEntryUpdatedDate(a).getTime();
		if (timeDifference !== 0) {
			return timeDifference;
		}

		return getEntryName(a).localeCompare(getEntryName(b));
	});

	// Filter by the currently selected tab in the frontend
	const filtered = filterSidebarEntries(pullRequests, selectedOption, result);

	return filtered;
}

function filterSidebarEntries(
	pullRequests: PullRequest[],
	selectedOption: string,
	sidebarEntries: SidebarEntrySubject[]
): SidebarEntrySubject[] {
	switch (selectedOption) {
		case 'pullRequest': {
			return sidebarEntries.filter(
				(sidebarEntry) =>
					sidebarEntry.type === 'pullRequest' ||
					pullRequests.some((pullRequest) =>
						containsPullRequestBranch(sidebarEntry.subject, pullRequest.sourceBranch)
					)
			);
		}
		case 'local': {
			return sidebarEntries.filter(
				(sidebarEntry) =>
					sidebarEntry.type === 'branchListing' &&
					(sidebarEntry.subject.hasLocal || sidebarEntry.subject.stack)
			);
		}
		default: {
			return sidebarEntries;
		}
	}
}

function containsPullRequestBranch(branchListing: BranchListing, sourceBranch: string): boolean {
	if (sourceBranch === branchListing.name) return true;
	if (branchListing.stack?.branches.includes(sourceBranch)) return true;
	return false;
}

export function groupBranches(branches: SidebarEntrySubject[], user: ForgeUser | undefined) {
	const grouped: GroupedSidebarEntries = {
		applied: [],
		authored: [],
		review: [],
		today: [],
		yesterday: [],
		lastWeek: [],
		older: []
	};

	const now = Date.now();

	for (let i = 0; i < branches.length; i++) {
		const b = branches[i];
		if (!b) continue;

		if (!getEntryUpdatedDate(b)) {
			grouped.older.push(b);
			continue;
		}

		const msSinceLastCommit = now - getEntryUpdatedDate(b).getTime();

		if (getEntryWorkspaceStatus(b)) {
			grouped.applied.push(b);
			continue;
		}

		if (isAuthoreOfEntry(user?.id, b)) {
			grouped.authored.push(b);
			continue;
		}

		if (isReviewerOfEntry(user?.id, b)) {
			grouped.review.push(b);
			continue;
		}

		if (msSinceLastCommit < msSinceDaysAgo(1)) {
			grouped.today.push(b);
			continue;
		}

		if (msSinceLastCommit < msSinceDaysAgo(2)) {
			grouped.yesterday.push(b);
			continue;
		}

		if (msSinceLastCommit < msSinceDaysAgo(7)) {
			grouped.lastWeek.push(b);
			continue;
		}

		grouped.older.push(b);
	}

	return grouped;
}

/** All branch names associated with listing. */
function getBranchNames(branchListing: BranchListing) {
	if (branchListing.stack) {
		return branchListing.stack.branches;
	} else {
		return [branchListing.name];
	}
}
