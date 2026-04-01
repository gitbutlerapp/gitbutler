import { mutationOptions } from "@tanstack/react-query";
import { rub } from "#ui/api/rub.ts";

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

export const rubMutationOptions = mutationOptions({
	mutationFn: rub,
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
