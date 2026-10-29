import {
	aggregateFileDependencies,
	filterDependenciesByAssignments,
	type FileDependencies,
} from "$lib/hunks/dependencies";
import { createSelectByIds } from "$lib/state/customSelectors";
import { InjectionToken } from "@gitbutler/core/context";
import { createEntityAdapter } from "@reduxjs/toolkit";
import type { WorktreeService } from "$lib/worktree/worktreeService.svelte";
import type { HunkDependencies } from "@gitbutler/but-sdk";

export const DEPENDENCY_SERVICE = new InjectionToken<DependencyService>("DependencyService");

export default class DependencyService {
	constructor(private readonly worktreeService: WorktreeService) {}

	fileDependencies(projectId: string, filePath: string, stackId?: string) {
		return this.worktreeService.worktreeChanges.useQuery(
			{ projectId },
			{
				transform: ({ dependencies, hunkAssignments }) => {
					if (!dependencies) {
						return {
							path: filePath,
							dependencies: [],
						};
					}

					const filtered = filterDependenciesByAssignments(dependencies, hunkAssignments, stackId);
					const e = toEntityAdapter(filtered);
					return (
						fileDependencySelectors.selectById(e.fileDependencies, filePath) || {
							path: filePath,
							dependencies: [],
						}
					);
				},
			},
		);
	}

	filesDependencies(projectId: string, filePaths: string[], stackId?: string) {
		return this.worktreeService.worktreeChanges.useQuery(
			{ projectId },
			{
				transform: ({ dependencies, hunkAssignments }) => {
					if (!dependencies) {
						return [];
					}

					const filtered = filterDependenciesByAssignments(dependencies, hunkAssignments, stackId);
					const e = toEntityAdapter(filtered);
					return fileDependencySelectors.selectByIds(e.fileDependencies, filePaths);
				},
			},
		);
	}
}

function toEntityAdapter(dependencies: HunkDependencies) {
	const [filePaths, fileDependencies] = aggregateFileDependencies(dependencies);

	return {
		filePaths,
		fileDependencies: fileDependenciesAdapter.addMany(
			fileDependenciesAdapter.getInitialState(),
			fileDependencies,
		),
	};
}

const fileDependenciesAdapter = createEntityAdapter<FileDependencies, string>({
	selectId: (fileDependency) => fileDependency.path,
});

const fileDependencySelectors = {
	...fileDependenciesAdapter.getSelectors(),
	selectByIds: createSelectByIds<FileDependencies>(),
};
