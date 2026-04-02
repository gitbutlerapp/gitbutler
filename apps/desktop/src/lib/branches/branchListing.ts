import { msSinceDaysAgo } from "$lib/utils/time";
import { isDefined } from "@gitbutler/ui/utils/typeguards";
import type { ForgeUser, PullRequest } from "$lib/forge/interface/types";
import type { BranchListing } from "@gitbutler/but-sdk";

export type GroupedSidebarEntries = Record<
	"applied" | "authored" | "review" | "today" | "yesterday" | "lastWeek" | "older",
	SidebarEntrySubject[]
>;

type PullRequestEntrySubject = {
	type: "pullRequest";
	subject: PullRequest;
	branchListing?: BranchListing;
};

type BranchListingEntrySubject = {
	type: "branchListing";
	subject: BranchListing;
	prs: PullRequest[];
};

export type SidebarEntrySubject = PullRequestEntrySubject | BranchListingEntrySubject;

export const BRANCH_FILTER_OPTIONS = ["all", "pullRequest", "local"] as const;
export type BranchFilterOption = (typeof BRANCH_FILTER_OPTIONS)[number];

export function isBranchFilterOption(something: unknown): something is BranchFilterOption {
	return (
		typeof something === "string" && BRANCH_FILTER_OPTIONS.includes(something as BranchFilterOption)
	);
}

function getEntryUpdatedDate(entry: SidebarEntrySubject) {
	return new Date(
		entry.type === "branchListing" ? entry.subject.updatedAt : entry.subject.modifiedAt,
	);
}

function getEntryName(entry: SidebarEntrySubject) {
	return entry.type === "branchListing" ? entry.subject.name : entry.subject.title;
}

function getEntryWorkspaceStatus(entry: SidebarEntrySubject) {
	return entry.type === "branchListing" ? entry.subject.stack?.inWorkspace : undefined;
}

function isReviewerOfEntry(userId: number | undefined, entry: SidebarEntrySubject): boolean {
	if (userId === undefined) return false;
	if (entry.type === "pullRequest") {
		return entry.subject.reviewers.some((r) => r.id === userId);
	}
	return entry.prs.some((pr) => pr.reviewers.some((r) => r.id === userId));
}

function isAuthoreOfEntry(userId: number | undefined, entry: SidebarEntrySubject): boolean {
	if (userId === undefined) return false;
	if (entry.type === "pullRequest") {
		return entry.subject.author?.id === userId;
	}
	return entry.prs.some((pr) => pr.author?.id === userId);
}

export function combineBranchesAndPrs(
	pullRequests: PullRequest[],
	branchList: BranchListing[],
	selectedOption: BranchFilterOption,
) {
	const prMap = Object.fromEntries(pullRequests.map((pr) => [pr.sourceBranch, pr]));

	const listingSubjects: BranchListingEntrySubject[] = branchList.map((subject) => ({
		type: "branchListing",
		subject: subject,
		prs:
			getBranchNames(subject)
				.map((name) => prMap[name])
				.filter(isDefined) || [],
	}));

	const attachedPrs = new Set(listingSubjects.flatMap((item) => item.prs.map((pr) => pr.number)));
	const prs: PullRequestEntrySubject[] = pullRequests
		.filter((pr) => !attachedPrs.has(pr.number))
		.map((pullRequests) => ({ type: "pullRequest", subject: pullRequests }));

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
	sidebarEntries: SidebarEntrySubject[],
): SidebarEntrySubject[] {
	switch (selectedOption) {
		case "pullRequest": {
			return sidebarEntries.filter(
				(sidebarEntry) =>
					sidebarEntry.type === "pullRequest" ||
					pullRequests.some((pullRequest) =>
						containsPullRequestBranch(sidebarEntry.subject, pullRequest.sourceBranch),
					),
			);
		}
		case "local": {
			return sidebarEntries.filter(
				(sidebarEntry) =>
					sidebarEntry.type === "branchListing" &&
					(sidebarEntry.subject.hasLocal || sidebarEntry.subject.stack),
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
		older: [],
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
