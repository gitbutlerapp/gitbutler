import {
	aggregateFileDependencies,
	type FileDependencies,
	type HunkDependencies
} from '$lib/dependencies/dependencies';
import { createSelectByIds } from '$lib/state/customSelectors';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type { BackendApi, ClientState } from '$lib/state/clientState.svelte';

export default class DependencyService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	fileDependencies(projectId: string, worktreeChangesKey: number, filePath: string) {
		return this.api.endpoints.dependencies.useQuery(
			{ projectId, worktreeChangesKey },
			{ transform: (dependencies) => fileDependencySelectors.selectById(dependencies, filePath) }
		);
	}

	filesDependencies(projectId: string, worktreeChangesKey: number, filePaths: string[]) {
		return this.api.endpoints.dependencies.useQuery(
			{ projectId, worktreeChangesKey },
			{
				transform: (dependencies) => fileDependencySelectors.selectByIds(dependencies, filePaths)
			}
		);
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			dependencies: build.query<
				EntityState<FileDependencies, string>,
				{ projectId: string; worktreeChangesKey: number }
			>({
				query: ({ projectId }) => ({
					params: { projectId },
					command: 'hunk_dependencies_for_workspace_changes'
				}),
				transformResponse(hunkDependencies: HunkDependencies) {
					const fileDependencies = aggregateFileDependencies(hunkDependencies);

					return fileDependenciesAdapter.addMany(
						fileDependenciesAdapter.getInitialState(),
						fileDependencies
					);
				}
			})
		})
	});
}

const fileDependenciesAdapter = createEntityAdapter<FileDependencies, string>({
	selectId: (fileDependency) => fileDependency.path
});

const fileDependencySelectors = {
	...fileDependenciesAdapter.getSelectors(),
	selectByIds: createSelectByIds<FileDependencies>()
};
