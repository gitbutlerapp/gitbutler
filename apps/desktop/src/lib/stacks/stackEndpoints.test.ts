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

	test("invalidates integration state after creating commits", () => {
		const endpoints = buildStackEndpoints(createEndpointBuilder());

		expect(endpoints.commitCreate.invalidatesTags).toEqual([
			invalidatesList(ReduxTag.WorktreeChanges),
			invalidatesList(ReduxTag.UpstreamIntegrationStatus),
			invalidatesList(ReduxTag.IntegrationSteps),
			invalidatesList(ReduxTag.HeadSha),
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

	test("uses get_initial_branch_integration with the branch ref", () => {
		const endpoints = buildStackEndpoints(createEndpointBuilder());
		const query = endpoints.getInitialBranchIntegration.query;

		expect(endpoints.getInitialBranchIntegration.extraOptions).toEqual({
			command: "get_initial_branch_integration",
		});
		expect(query).toBeDefined();
		expect(
			query?.({
				projectId: "project-1",
				branchRef: "refs/heads/feature",
				strategy: "merge",
			}),
		).toEqual({
			projectId: "project-1",
			branch: "refs/heads/feature",
			strategy: "merge",
		});
	});

	test("uses apply_branch_integration with dryRun previews and branch refs", () => {
		const endpoints = buildStackEndpoints(createEndpointBuilder());
		const query = endpoints.applyBranchIntegration.query;
		const invalidatesTags = endpoints.applyBranchIntegration.invalidatesTags;
		const previewArgs = {
			projectId: "project-1",
			branchRef: "refs/heads/feature",
			integration: {
				mergeBase: "1111111111111111111111111111111111111111",
				firstLocalNotIntegrated: null,
				steps: [{ kind: "pick" as const, commitId: "2222222222222222222222222222222222222222" }],
			},
			dryRun: true,
		};
		const applyArgs = { ...previewArgs, dryRun: false };

		expect(endpoints.applyBranchIntegration.extraOptions).toEqual({
			command: "apply_branch_integration",
			actionName: "Apply Branch Integration",
		});
		expect(query).toBeDefined();
		expect(query?.(previewArgs)).toEqual({
			projectId: "project-1",
			branch: "refs/heads/feature",
			integration: previewArgs.integration,
			dryRun: true,
		});

		if (typeof invalidatesTags !== "function") {
			throw new Error("Expected applyBranchIntegration.invalidatesTags to be callable");
		}

		expect(invalidatesTags(undefined, undefined, previewArgs, undefined)).toEqual([]);
		expect(invalidatesTags(undefined, undefined, applyArgs, undefined)).toEqual([
			invalidatesList(ReduxTag.HeadSha),
			invalidatesList(ReduxTag.WorktreeChanges),
			invalidatesList(ReduxTag.Stacks),
			invalidatesList(ReduxTag.StackDetails),
			invalidatesList(ReduxTag.BranchListing),
			invalidatesList(ReduxTag.UpstreamIntegrationStatus),
			invalidatesItem(ReduxTag.IntegrationSteps, "refs/heads/feature"),
		]);
	});

	test("uses workspace_integrate_upstream for dry-run previews and execution", () => {
		const endpoints = buildStackEndpoints(createEndpointBuilder());
		const query = endpoints.workspaceIntegrateUpstream.query;
		const invalidatesTags = endpoints.workspaceIntegrateUpstream.invalidatesTags;
		const previewArgs = {
			projectId: "project-1",
			updates: [
				{
					kind: "rebase" as const,
					selector: {
						type: "commit" as const,
						subject: "1111111111111111111111111111111111111111",
					},
				},
			],
			dryRun: true,
		};
		const executeArgs = { ...previewArgs, dryRun: false };

		expect(endpoints.workspaceIntegrateUpstream.extraOptions).toEqual({
			command: "workspace_integrate_upstream",
			actionName: "Update Workspace",
		});
		expect(query).toBeDefined();
		expect(query?.(previewArgs)).toEqual(previewArgs);

		if (typeof invalidatesTags !== "function") {
			throw new Error("Expected workspaceIntegrateUpstream.invalidatesTags to be callable");
		}

		expect(invalidatesTags(undefined, undefined, previewArgs, undefined)).toEqual([]);
		expect(invalidatesTags(undefined, undefined, executeArgs, undefined)).toEqual([
			invalidatesList(ReduxTag.HeadSha),
			invalidatesList(ReduxTag.WorktreeChanges),
			invalidatesList(ReduxTag.Stacks),
			invalidatesList(ReduxTag.StackDetails),
			invalidatesList(ReduxTag.BranchChanges),
			invalidatesList(ReduxTag.BranchListing),
			invalidatesList(ReduxTag.BaseBranchData),
			invalidatesList(ReduxTag.UpstreamIntegrationStatus),
		]);
	});
});
