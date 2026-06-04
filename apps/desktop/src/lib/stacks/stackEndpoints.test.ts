import { buildStackEndpoints } from "$lib/stacks/stackEndpoints";
import { invalidatesItem, invalidatesList, ReduxTag } from "$lib/state/tags";
import { describe, expect, test } from "vitest";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";

function createEndpointBuilder(): BackendEndpointBuilder {
	return {
		mutation: (definition) => definition,
		query: (definition) => definition,
	} as BackendEndpointBuilder;
}

describe("buildStackEndpoints", () => {
	test("maps uncommit to commit_uncommit with the new request shape", () => {
		const endpoints = buildStackEndpoints(createEndpointBuilder());
		const query = endpoints.uncommit.query;

		expect(endpoints.uncommit.extraOptions).toEqual({
			command: "commit_uncommit",
			actionName: "Uncommit",
		});
		expect(query).toBeDefined();
		expect(
			query?.({
				projectId: "project-1",
				stackId: "stack-1",
				commitIds: ["commit-1"],
			}),
		).toEqual({
			projectId: "project-1",
			subjectCommitIds: ["commit-1"],
			assignTo: "stack-1",
			dryRun: false,
		});
	});

	test("maps squash commits to commit_squash with the new request shape", () => {
		const endpoints = buildStackEndpoints(createEndpointBuilder());
		const query = endpoints.squashCommits.query;

		expect(endpoints.squashCommits.extraOptions).toEqual({
			command: "commit_squash",
			actionName: "Squash Commits",
		});
		expect(query).toBeDefined();
		expect(
			query?.({
				projectId: "project-1",
				sourceCommitIds: ["commit-1", "commit-2"],
				targetCommitId: "commit-3",
			}),
		).toEqual({
			projectId: "project-1",
			subjectCommitIds: ["commit-1", "commit-2"],
			targetCommitId: "commit-3",
			howToCombineMessages: "KeepBoth",
			dryRun: false,
		});
	});

	test("uses commit_move for generic commit moves", () => {
		const endpoints = buildStackEndpoints(createEndpointBuilder());
		const query = endpoints.commitMove.query;

		expect(endpoints.commitMove.extraOptions).toEqual({
			command: "commit_move",
			actionName: "Move Commit",
		});
		expect(query).toBeDefined();
		expect(
			query?.({
				projectId: "project-1",
				subjectCommitIds: ["commit-1"],
				relativeTo: {
					type: "commit",
					subject: "commit-2",
				},
				side: "below",
				dryRun: false,
			}),
		).toEqual({
			projectId: "project-1",
			subjectCommitIds: ["commit-1"],
			relativeTo: { type: "commit", subject: "commit-2" },
			side: "below",
			dryRun: false,
		});
	});

	test("invalidates branch and worktree state after commit moves", () => {
		const endpoints = buildStackEndpoints(createEndpointBuilder());

		expect(endpoints.commitMove.invalidatesTags).toEqual([
			invalidatesList(ReduxTag.HeadSha),
			invalidatesList(ReduxTag.WorktreeChanges),
			invalidatesList(ReduxTag.BranchChanges),
			invalidatesList(ReduxTag.Stacks),
			invalidatesList(ReduxTag.StackDetails),
		]);
	});

	test("uses move_branch with normalized refs and dryRun disabled", () => {
		const endpoints = buildStackEndpoints(createEndpointBuilder());
		const query = endpoints.moveBranch.query;

		expect(endpoints.moveBranch.extraOptions).toEqual({
			command: "move_branch",
			actionName: "Move Branch",
		});
		expect(query).toBeDefined();
		expect(
			query?.({
				projectId: "project-1",
				subjectBranch: "refs/heads/feature/source",
				targetBranch: "refs/heads/feature/target",
			}),
		).toEqual({
			projectId: "project-1",
			subjectBranch: "refs/heads/feature/source",
			targetBranch: "refs/heads/feature/target",
			dryRun: false,
		});
	});

	test("uses tear_off_branch with normalized refs and dryRun disabled", () => {
		const endpoints = buildStackEndpoints(createEndpointBuilder());
		const query = endpoints.tearOffBranch.query;
		const invalidatesTags = endpoints.tearOffBranch.invalidatesTags;
		const args = {
			projectId: "project-1",
			sourceStackId: "stack-1",
			subjectBranchName: "feature/source",
		};

		expect(endpoints.tearOffBranch.extraOptions).toEqual({
			command: "tear_off_branch",
			actionName: "Tear Off Branch",
		});
		expect(query).toBeDefined();
		expect(query?.(args)).toEqual({
			projectId: "project-1",
			subjectBranch: "refs/heads/feature/source",
			dryRun: false,
		});

		if (typeof invalidatesTags !== "function") {
			throw new Error("Expected tearOffBranch.invalidatesTags to be callable");
		}

		expect(invalidatesTags(undefined, undefined, args, undefined)).toEqual([
			invalidatesList(ReduxTag.HeadSha),
			invalidatesList(ReduxTag.WorktreeChanges),
			invalidatesList(ReduxTag.Stacks),
			invalidatesList(ReduxTag.BranchChanges),
			invalidatesItem(ReduxTag.StackDetails, "stack-1"),
		]);
	});
});
