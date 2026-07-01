import { worktreeSelectors } from "$lib/worktree/worktreeEndpoints";
import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/backendApi";

export const WORKTREE_SERVICE = new InjectionToken<WorktreeService>("WorktreeService");

/**
 * A service for tracking uncommitted changes.
 *
 * Since we want to maintain a list and access individual records we use a
 * redux entity adapter on the results.
 */
export class WorktreeService {
	constructor(private backendApi: BackendApi) {}

	treeChanges(projectId: string) {
		return this.backendApi.endpoints.worktreeChanges.useQuery(
			{ projectId },
			{ transform: (res) => res.rawChanges },
		);
	}

	hunkAssignments(projectId: string) {
		return this.backendApi.endpoints.worktreeChanges.useQuery(
			{ projectId },
			{ transform: (res) => res.hunkAssignments },
		);
	}

	worktreeData(projectId: string) {
		return this.backendApi.endpoints.worktreeChanges.useQuery({ projectId });
	}

	treeChangeByPath(projectId: string, path: string) {
		const { worktreeChanges: getChanges } = this.backendApi.endpoints;
		return getChanges.useQueryState(
			{ projectId },
			{ transform: (res) => worktreeSelectors.selectById(res.changes, path)! },
		);
	}

	treeChangesByPaths(projectId: string, paths: string[]) {
		const { worktreeChanges: getChanges } = this.backendApi.endpoints;
		return getChanges.useQueryState(
			{ projectId },
			{ transform: (res) => worktreeSelectors.selectByIds(res.changes, paths) },
		);
	}

	async fetchTreeChange(projectId: string, path: string) {
		const { worktreeChanges } = this.backendApi.endpoints;
		return await worktreeChanges.fetch(
			{ projectId },
			{ transform: (res) => worktreeSelectors.selectById(res.changes, path)! },
		);
	}

	/**
	 * Exposes the worktreeChanges endpoint. This is currently intended to be
	 * consumed by just the `DependencyService`.
	 */
	get worktreeChanges() {
		return this.backendApi.endpoints.worktreeChanges;
	}
}
