import { createEntityAdapter, type EntityState } from "@reduxjs/toolkit";
import type {
	Commit,
	RefInfo,
	Segment,
	Stack as RefInfoStack,
	UpstreamCommit,
} from "@gitbutler/but-sdk";

export type WorkspaceStackDetails = {
	stack: RefInfoStack;
	segments: Segment[];
	commits: EntityState<Commit, string>;
	upstreamCommits: EntityState<UpstreamCommit, string>;
};

export type WorkspaceDetails = {
	stacks: EntityState<RefInfoStack, string>;
	stackDetails: Record<string, WorkspaceStackDetails>;
};

const NULL_SHA = "0000000000000000000000000000000000000000";

const stackAdapter = createEntityAdapter<RefInfoStack, string>({
	selectId: (stack) => stackKey(stack),
});

const commitAdapter = createEntityAdapter<Commit, string>({
	selectId: (commit) => commit.id,
});

const upstreamCommitAdapter = createEntityAdapter<UpstreamCommit, string>({
	selectId: (commit) => commit.id,
});

function stackKey(stack: RefInfoStack): string {
	return stack.id ?? stack.segments.at(0)?.refName?.displayName ?? stack.base ?? NULL_SHA;
}

export function transformWorkspaceDetails(response: RefInfo): WorkspaceDetails {
	const stackDetails = Object.fromEntries(
		response.stacks.map((stack) => {
			return [
				stackKey(stack),
				{
					stack,
					segments: stack.segments,
					commits: commitAdapter.addMany(
						commitAdapter.getInitialState(),
						stack.segments.flatMap((segment) => segment.commits),
					),
					upstreamCommits: upstreamCommitAdapter.addMany(
						upstreamCommitAdapter.getInitialState(),
						stack.segments.flatMap((segment) => segment.commitsOnRemote),
					),
				},
			];
		}),
	);

	return {
		stacks: stackAdapter.addMany(stackAdapter.getInitialState(), response.stacks),
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
			details.segments.some((segment) => segment.refName?.displayName === branchName),
		);
	}
	return Object.values(workspaceDetails.stackDetails).at(0);
}

export function selectWorkspaceStackById(
	workspaceDetails: WorkspaceDetails,
	stackId: string,
): RefInfoStack | undefined {
	return stackAdapter.getSelectors().selectById(workspaceDetails.stacks, stackId);
}

export function workspaceStackDetailTags(workspaceDetails: WorkspaceDetails): string[] {
	return Object.keys(workspaceDetails.stackDetails);
}
