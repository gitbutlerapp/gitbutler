import { mutationOptions } from "@tanstack/react-query";
import {
	changesInWorktreeQueryOptions,
	headInfoQueryOptions,
	listBranchesQueryOptions,
} from "#ui/queries.ts";
import { rub } from "#ui/rub.ts";

export const applyBranchMutationOptions = mutationOptions({
	mutationFn: window.lite.apply,
	onSuccess: async (_data, { projectId }, _ctx, { client }) => {
		await Promise.all([
			client.invalidateQueries(listBranchesQueryOptions(projectId)),
			client.invalidateQueries(headInfoQueryOptions(projectId)),
			client.invalidateQueries(changesInWorktreeQueryOptions(projectId)),
		]);
	},
});

export const commitInsertBlankMutationOptions = mutationOptions({
	mutationFn: window.lite.commitInsertBlank,
	onSuccess: async (_data, { projectId }, _ctx, { client }) => {
		await Promise.all([
			client.invalidateQueries(headInfoQueryOptions(projectId)),
			client.invalidateQueries(changesInWorktreeQueryOptions(projectId)),
			client.invalidateQueries(listBranchesQueryOptions(projectId)),
			client.invalidateQueries({ queryKey: ["branchDetails"] }),
			client.invalidateQueries({ queryKey: ["branchDiff"] }),
			client.invalidateQueries({ queryKey: ["commitDetailsWithLineStats"] }),
		]);
	},
});

export const commitMoveMutationOptions = mutationOptions({
	mutationFn: window.lite.commitMove,
	onSuccess: async (_data, { projectId }, _ctx, { client }) => {
		await Promise.all([
			client.invalidateQueries(headInfoQueryOptions(projectId)),
			client.invalidateQueries(changesInWorktreeQueryOptions(projectId)),
			client.invalidateQueries(listBranchesQueryOptions(projectId)),
			client.invalidateQueries({ queryKey: ["branchDetails"] }),
			client.invalidateQueries({ queryKey: ["branchDiff"] }),
			client.invalidateQueries({ queryKey: ["commitDetailsWithLineStats"] }),
		]);
	},
});

export const commitMoveToBranchMutationOptions = mutationOptions({
	mutationFn: window.lite.commitMoveToBranch,
	onSuccess: async (_data, { projectId }, _ctx, { client }) => {
		await Promise.all([
			client.invalidateQueries(headInfoQueryOptions(projectId)),
			client.invalidateQueries(changesInWorktreeQueryOptions(projectId)),
			client.invalidateQueries(listBranchesQueryOptions(projectId)),
			client.invalidateQueries({ queryKey: ["branchDetails"] }),
			client.invalidateQueries({ queryKey: ["branchDiff"] }),
			client.invalidateQueries({ queryKey: ["commitDetailsWithLineStats"] }),
		]);
	},
});

export const commitMutationOptions = mutationOptions({
	mutationFn: window.lite.commitCreate,
	onSuccess: async (_data, { projectId }, _ctx, { client }) => {
		await Promise.all([
			client.invalidateQueries(headInfoQueryOptions(projectId)),
			client.invalidateQueries(changesInWorktreeQueryOptions(projectId)),
		]);
	},
});

export const commitRewordMutationOptions = mutationOptions({
	mutationFn: window.lite.commitReword,
	onSuccess: async (_data, { projectId }, _ctx, { client }) => {
		await Promise.all([
			client.invalidateQueries(headInfoQueryOptions(projectId)),
			client.invalidateQueries(changesInWorktreeQueryOptions(projectId)),
			client.invalidateQueries(listBranchesQueryOptions(projectId)),
			client.invalidateQueries({ queryKey: ["branchDetails"] }),
			client.invalidateQueries({ queryKey: ["branchDiff"] }),
			client.invalidateQueries({ queryKey: ["commitDetailsWithLineStats"] }),
		]);
	},
});

export const rubMutationOptions = mutationOptions({
	mutationFn: rub,
	onSuccess: async (_data, { projectId }, _ctx, { client }) => {
		await Promise.all([
			client.invalidateQueries(headInfoQueryOptions(projectId)),
			client.invalidateQueries(changesInWorktreeQueryOptions(projectId)),
		]);
	},
});

export const unapplyStackMutationOptions = mutationOptions({
	mutationFn: window.lite.unapplyStack,
	onSuccess: async (_data, { projectId }, _ctx, { client }) => {
		await Promise.all([
			client.invalidateQueries(listBranchesQueryOptions(projectId)),
			client.invalidateQueries(headInfoQueryOptions(projectId)),
			client.invalidateQueries(changesInWorktreeQueryOptions(projectId)),
		]);
	},
});
