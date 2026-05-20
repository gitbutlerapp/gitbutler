import { createEntityAdapter, type EntityState } from "@reduxjs/toolkit";
import type {
	BranchDetails,
	Commit,
	RefInfo,
	Segment,
	Stack as RefInfoStack,
	StackDetails,
	StackEntry,
	UpstreamCommit,
} from "@gitbutler/but-sdk";

export type WorkspaceStackDetails = {
	stackInfo: StackDetails;
	branchDetails: EntityState<BranchDetails, string>;
	commits: EntityState<Commit, string>;
	upstreamCommits: EntityState<UpstreamCommit, string>;
};

export type WorkspaceDetails = {
	stacks: EntityState<StackEntry, string>;
	stackDetails: Record<string, WorkspaceStackDetails>;
};

const NULL_SHA = "0000000000000000000000000000000000000000";

const stackAdapter = createEntityAdapter<StackEntry, string>({
	selectId: (stack) => stack.id ?? stack.heads.at(0)?.name ?? stack.tip,
});

const branchDetailsAdapter = createEntityAdapter<BranchDetails, string>({
	selectId: (branch) => branch.name,
});

const commitAdapter = createEntityAdapter<Commit, string>({
	selectId: (commit) => commit.id,
});

const upstreamCommitAdapter = createEntityAdapter<UpstreamCommit, string>({
	selectId: (commit) => commit.id,
});

function decodeBytes(bytes: number[]): string {
	return new TextDecoder().decode(new Uint8Array(bytes));
}

function stackKey(stack: StackEntry): string {
	return stack.id ?? stack.heads.at(0)?.name ?? stack.tip;
}

function segmentName(segment: Segment): string {
	if (!segment.refName) {
		throw new Error("Cannot map anonymous head_info segment to legacy branch details");
	}
	return segment.refName.displayName;
}

function segmentReference(segment: Segment): string {
	if (!segment.refName) {
		throw new Error("Cannot map anonymous head_info segment to legacy branch details");
	}
	return decodeBytes(segment.refName.fullNameBytes);
}

function segmentTip(stack: RefInfoStack, segment: Segment): string {
	return segment.commits.at(0)?.id ?? segment.base ?? stack.base ?? NULL_SHA;
}

function branchLastUpdatedAt(segment: Segment): number | null {
	const seconds = segment.metadata?.refInfo.updatedAt?.seconds;
	return seconds === undefined ? null : seconds * 1000;
}

function branchAuthors(segment: Segment): BranchDetails["authors"] {
	const authors = [...segment.commits, ...segment.commitsOnRemote].map((commit) => commit.author);
	return Array.from(
		new Map(authors.map((author) => [JSON.stringify(author), author])).values(),
	).sort((a, b) => (a.name ?? "").localeCompare(b.name ?? ""));
}

export function stackEntryFromHeadInfoStack(stack: RefInfoStack, index: number): StackEntry {
	const heads = stack.segments.map((segment) => ({
		name: segmentName(segment),
		tip: segmentTip(stack, segment),
		reviewId: segment.metadata?.review.pullRequest ?? null,
		isCheckedOut: segment.isEntrypoint,
	}));
	const tip = heads.at(0)?.tip ?? stack.base ?? NULL_SHA;

	return {
		id: stack.id,
		heads,
		tip,
		order: index,
		isCheckedOut: heads.some((head) => head.isCheckedOut),
	};
}

export function stackDetailsFromHeadInfoStack(stack: RefInfoStack): StackDetails {
	const branchDetails = stack.segments.map((segment): BranchDetails => {
		const baseCommit = segment.base ?? stack.base ?? NULL_SHA;
		const remoteTrackingBranch = segment.remoteTrackingRefName
			? decodeBytes(segment.remoteTrackingRefName.fullNameBytes)
			: null;

		return {
			name: segmentName(segment),
			reference: segmentReference(segment),
			linkedWorktreeId: null,
			remoteTrackingBranch,
			prNumber: segment.metadata?.review.pullRequest ?? null,
			reviewId: segment.metadata?.review.reviewId ?? null,
			tip: segmentTip(stack, segment),
			baseCommit,
			pushStatus: segment.pushStatus,
			lastUpdatedAt: branchLastUpdatedAt(segment),
			authors: branchAuthors(segment),
			isConflicted: segment.commits.some((commit) => commit.hasConflicts),
			commits: segment.commits,
			upstreamCommits: segment.commitsOnRemote,
			isRemoteHead: segmentReference(segment).startsWith("refs/remotes/"),
		};
	});
	const topmostBranch = branchDetails.at(0);

	if (!topmostBranch) {
		throw new Error("Cannot map empty head_info stack to legacy stack details");
	}

	return {
		derivedName: topmostBranch.name,
		pushStatus: topmostBranch.pushStatus,
		branchDetails,
		isConflicted: topmostBranch.isConflicted,
	};
}

export function transformWorkspaceDetails(response: RefInfo): WorkspaceDetails {
	const stacks = response.stacks.map(stackEntryFromHeadInfoStack);
	const stackDetails = Object.fromEntries(
		response.stacks.map((stack, index) => {
			const entry = stacks[index]!;
			const details = stackDetailsFromHeadInfoStack(stack);
			return [
				stackKey(entry),
				{
					stackInfo: details,
					branchDetails: branchDetailsAdapter.addMany(
						branchDetailsAdapter.getInitialState(),
						details.branchDetails,
					),
					commits: commitAdapter.addMany(
						commitAdapter.getInitialState(),
						details.branchDetails.flatMap((branch) => branch.commits),
					),
					upstreamCommits: upstreamCommitAdapter.addMany(
						upstreamCommitAdapter.getInitialState(),
						details.branchDetails.flatMap((branch) => branch.upstreamCommits),
					),
				},
			];
		}),
	);

	return {
		stacks: stackAdapter.addMany(stackAdapter.getInitialState(), stacks),
		stackDetails,
	};
}

export function selectWorkspaceStackDetails(
	workspaceDetails: WorkspaceDetails,
	stackId?: string,
	branchName?: string,
): WorkspaceStackDetails | undefined {
	if (stackId) return workspaceDetails.stackDetails[stackId];
	if (branchName) {
		return Object.values(workspaceDetails.stackDetails).find((details) =>
			details.stackInfo.branchDetails.some((branch) => branch.name === branchName),
		);
	}
	return Object.values(workspaceDetails.stackDetails).at(0);
}

export function selectWorkspaceStackById(
	workspaceDetails: WorkspaceDetails,
	stackId: string,
): StackEntry | undefined {
	return stackAdapter.getSelectors().selectById(workspaceDetails.stacks, stackId);
}

export function workspaceStackDetailTags(workspaceDetails: WorkspaceDetails): string[] {
	return Object.keys(workspaceDetails.stackDetails);
}
