import {
	aggregateFileDependencies,
	type FileDependencies,
	type HunkDependencies
} from '$lib/dependencies/dependencies';
import { createSelectByIds } from '$lib/state/customSelectors';
import { InjectionToken } from '@gitbutler/core/context';
import { createEntityAdapter } from '@reduxjs/toolkit';
import type { WorktreeService } from '$lib/worktree/worktreeService.svelte';

export const DEPENDENCY_SERVICE = new InjectionToken<DependencyService>('DependencyService');

export default class DependencyService {
	constructor(private readonly worktreeService: WorktreeService) {}

	fileDependencies(projectId: string, filePath: string) {
		return this.worktreeService.worktreeChanges.useQuery(
			{ projectId },
			{
				transform: ({ dependencies }) => {
					if (!dependencies) {
						return {
							path: filePath,
							dependencies: []
						};
					}

					const e = toEntityAdapter(dependencies);
					return (
						fileDependencySelectors.selectById(e.fileDependencies, filePath) || {
							path: filePath,
							dependencies: []
						}
					);
				}
			}
		);
	}

	filesDependencies(projectId: string, filePaths: string[]) {
		return this.worktreeService.worktreeChanges.useQuery(
			{ projectId },
			{
				transform: ({ dependencies }) => {
					if (!dependencies) {
						return [];
					}

					const e = toEntityAdapter(dependencies);
					return fileDependencySelectors.selectByIds(e.fileDependencies, filePaths);
				}
			}
		);
	}
}

function toEntityAdapter(dependencies: HunkDependencies) {
	const [filePaths, fileDependencies] = aggregateFileDependencies(dependencies);

	return {
		filePaths,
		fileDependencies: fileDependenciesAdapter.addMany(
			fileDependenciesAdapter.getInitialState(),
			fileDependencies
		)
	};
}

const fileDependenciesAdapter = createEntityAdapter<FileDependencies, string>({
	selectId: (fileDependency) => fileDependency.path
});

const fileDependencySelectors = {
	...fileDependenciesAdapter.getSelectors(),
	selectByIds: createSelectByIds<FileDependencies>()
};
