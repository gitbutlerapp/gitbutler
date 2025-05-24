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
			{
				transform: ({ fileDependencies }) =>
					fileDependencySelectors.selectById(fileDependencies, filePath)
			}
		);
	}

	filesDependencies(projectId: string, worktreeChangesKey: number, filePaths: string[]) {
		return this.api.endpoints.dependencies.useQuery(
			{ projectId, worktreeChangesKey },
			{
				transform: ({ fileDependencies }) =>
					fileDependencySelectors.selectByIds(fileDependencies, filePaths)
			}
		);
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			dependencies: build.query<
				{ fileDependencies: EntityState<FileDependencies, string>; filePaths: string[] },
				{ projectId: string; worktreeChangesKey: number }
			>({
				query: ({ projectId }) => ({
					params: { projectId },
					command: 'hunk_dependencies_for_workspace_changes'
				}),
				transformResponse(hunkDependencies: HunkDependencies) {
					const [filePaths, fileDependencies] = aggregateFileDependencies(hunkDependencies);

					return {
						filePaths,
						fileDependencies: fileDependenciesAdapter.addMany(
							fileDependenciesAdapter.getInitialState(),
							fileDependencies
						)
					};
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
