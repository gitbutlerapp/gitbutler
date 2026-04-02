import { mutationOptions } from "@tanstack/react-query";

// TODO: replace with generated type when it becomes available
export type CommitUncommitParams = {
	projectId: string;
	commitId: string;
	assignTo: string | null;
};

// TODO: replace with generated type when it becomes available
export type CommitSquashParams = {
	projectId: string;
	sourceCommitId: string;
	destinationCommitId: string;
};

export const applyBranchMutationOptions = mutationOptions({
	mutationFn: window.lite.apply,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const absorptionPlanMutationOptions = mutationOptions({
	mutationFn: window.lite.absorptionPlan,
});

export const absorbMutationOptions = mutationOptions({
	mutationFn: window.lite.absorb,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const assignHunkMutationOptions = mutationOptions({
	mutationFn: window.lite.assignHunk,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const commitAmendMutationOptions = mutationOptions({
	mutationFn: window.lite.commitAmend,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const commitInsertBlankMutationOptions = mutationOptions({
	mutationFn: window.lite.commitInsertBlank,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const commitMoveMutationOptions = mutationOptions({
	mutationFn: window.lite.commitMove,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const commitMoveChangesBetweenMutationOptions = mutationOptions({
	mutationFn: window.lite.commitMoveChangesBetween,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const commitSquashMutationOptions = mutationOptions({
	mutationFn: async (_input: CommitSquashParams) => {
		throw new Error("Squashing has not been implemented yet.");
	},
});

export const commitUncommitMutationOptions = mutationOptions({
	mutationFn: async (_input: CommitUncommitParams) => {
		throw new Error("Uncommitting has not been implemented yet.");
	},
});

export const commitUncommitChangesMutationOptions = mutationOptions({
	mutationFn: window.lite.commitUncommitChanges,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const commitCreateMutationOptions = mutationOptions({
	mutationFn: window.lite.commitCreate,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const commitDiscardMutationOptions = mutationOptions({
	mutationFn: window.lite.commitDiscard,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const commitRewordMutationOptions = mutationOptions({
	mutationFn: window.lite.commitReword,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const moveBranchMutationOptions = mutationOptions({
	mutationFn: window.lite.moveBranch,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const removeBranchMutationOptions = mutationOptions({
	mutationFn: window.lite.removeBranch,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const updateBranchNameMutationOptions = mutationOptions({
	mutationFn: window.lite.updateBranchName,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const tearOffBranchMutationOptions = mutationOptions({
	mutationFn: window.lite.tearOffBranch,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const unapplyStackMutationOptions = mutationOptions({
	mutationFn: window.lite.unapplyStack,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});
