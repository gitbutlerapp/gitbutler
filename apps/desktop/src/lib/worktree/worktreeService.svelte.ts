import { worktreeSelectors, type WorktreeData } from "$lib/worktree/worktreeEndpoints";
import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/backendApi";
import type { TreeChange } from "@gitbutler/but-sdk";

export const WORKTREE_SERVICE = new InjectionToken<WorktreeService>("WorktreeService");

type LocalIgnoreOverride = {
	ignored: string[];
	unignored: string[];
};

export function normalizeLocalIgnorePath(path: string): string | undefined {
	const parts = path
		.replaceAll("\\", "/")
		.split("/")
		.filter((part) => part.length > 0 && part !== ".");
	if (parts.some((part) => part === "..")) return undefined;
	return parts.length > 0 ? parts.join("/") : undefined;
}

export function pathIsLocallyIgnored(path: string, ignoredPaths: string[]): boolean {
	const normalizedPath = normalizeLocalIgnorePath(path);
	if (!normalizedPath) return false;

	return ignoredPaths.some((ignoredPath) => {
		const normalizedIgnoredPath = normalizeLocalIgnorePath(ignoredPath);
		return (
			normalizedPath === normalizedIgnoredPath ||
			(!!normalizedIgnoredPath?.length && normalizedPath.startsWith(`${normalizedIgnoredPath}/`))
		);
	});
}

/**
 * A service for tracking uncommitted changes.
 *
 * Since we want to maintain a list and access individual records we use a
 * redux entity adapter on the results.
 */
export class WorktreeService {
	private localIgnoreOverrides = $state.raw<Record<string, LocalIgnoreOverride>>({});

	constructor(private backendApi: BackendApi) {}

	treeChanges(projectId: string) {
		return this.backendApi.endpoints.worktreeChanges.useQuery(
			{ projectId },
			{ transform: (res) => this.filterLocallyIgnoredWorktreeData(projectId, res).rawChanges },
		);
	}

	hunkAssignments(projectId: string) {
		return this.backendApi.endpoints.worktreeChanges.useQuery(
			{ projectId },
			{
				transform: (res) => this.filterLocallyIgnoredWorktreeData(projectId, res).hunkAssignments,
			},
		);
	}

	worktreeData(projectId: string) {
		return this.backendApi.endpoints.worktreeChanges.useQuery(
			{ projectId },
			{ transform: (res) => this.filterLocallyIgnoredWorktreeData(projectId, res) },
		);
	}

	unfilteredWorktreeData(projectId: string) {
		return this.backendApi.endpoints.worktreeChanges.useQuery({ projectId });
	}

	localIgnoredPaths(projectId: string) {
		return this.backendApi.endpoints.localIgnoredPaths.useQuery(
			{ projectId },
			{ transform: (paths) => this.applyLocalIgnoreOverrides(projectId, paths) },
		);
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

	async setLocalIgnoredPath(projectId: string, path: string, ignored: boolean) {
		const previousOverrides = this.localIgnoreOverrides;
		this.setLocalIgnoreOverride(projectId, path, ignored);
		try {
			await this.backendApi.endpoints.setLocalIgnoredPath.mutate({ projectId, path, ignored });
		} catch (error) {
			this.localIgnoreOverrides = previousOverrides;
			throw error;
		}
	}

	/**
	 * Exposes the worktreeChanges endpoint. This is currently intended to be
	 * consumed by just the `DependencyService`.
	 */
	get worktreeChanges() {
		return this.backendApi.endpoints.worktreeChanges;
	}

	private overrideForProject(projectId: string): LocalIgnoreOverride {
		return this.localIgnoreOverrides[projectId] ?? { ignored: [], unignored: [] };
	}

	private setLocalIgnoreOverride(projectId: string, path: string, ignored: boolean) {
		const normalizedPath = normalizeLocalIgnorePath(path);
		if (!normalizedPath) return;

		const current = this.overrideForProject(projectId);
		const next: LocalIgnoreOverride = ignored
			? {
					ignored: addPath(current.ignored, normalizedPath),
					unignored: removePath(current.unignored, normalizedPath),
				}
			: {
					ignored: removePath(current.ignored, normalizedPath),
					unignored: addPath(current.unignored, normalizedPath),
				};

		this.localIgnoreOverrides = {
			...this.localIgnoreOverrides,
			[projectId]: next,
		};
	}

	private applyLocalIgnoreOverrides(projectId: string, paths: string[]): string[] {
		const overrides = this.overrideForProject(projectId);
		const pathSet = new Set(
			paths.map(normalizeLocalIgnorePath).filter((path): path is string => !!path),
		);
		for (const path of overrides.ignored) {
			pathSet.add(path);
		}
		for (const path of overrides.unignored) {
			pathSet.delete(path);
		}
		return Array.from(pathSet).sort();
	}

	private filterLocallyIgnoredWorktreeData(projectId: string, data: WorktreeData): WorktreeData {
		const overrides = this.overrideForProject(projectId);
		if (overrides.ignored.length === 0) return data;

		const rawChanges = data.rawChanges.filter(
			(change) => !pathIsLocallyIgnored(change.path, overrides.ignored),
		);
		const ignoredChanges = data.ignoredChanges.filter(
			(change) => !pathIsLocallyIgnored(change.path, overrides.ignored),
		);
		const hunkAssignments = data.hunkAssignments.filter(
			(assignment) => !pathIsLocallyIgnored(assignment.path, overrides.ignored),
		);

		return {
			...data,
			rawChanges,
			ignoredChanges,
			hunkAssignments,
			changes: filterTreeChangesEntity(data.changes, overrides.ignored),
		};
	}
}

function addPath(paths: string[], path: string): string[] {
	return paths.includes(path) ? paths : [...paths, path];
}

function removePath(paths: string[], path: string): string[] {
	return paths.filter((item) => item !== path);
}

function filterTreeChangesEntity(
	changes: WorktreeData["changes"],
	ignoredPaths: string[],
): WorktreeData["changes"] {
	const filtered = worktreeSelectors
		.selectAll(changes)
		.filter((change) => !pathIsLocallyIgnored(change.path, ignoredPaths));
	return {
		ids: filtered.map((change) => change.path),
		entities: Object.fromEntries(filtered.map((change) => [change.path, change])) as Record<
			string,
			TreeChange
		>,
	};
}
