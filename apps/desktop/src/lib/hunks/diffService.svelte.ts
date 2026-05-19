import { InjectionToken } from "@gitbutler/core/context";
import { isDefined } from "@gitbutler/ui/utils/typeguards";
import type { UnitySemanticDiff } from "$lib/files/unitySemantic";
import type { UnifiedDiff } from "$lib/hunks/diff";
import type { BackendApi } from "$lib/state/backendApi";
import type { TreeChange } from "@gitbutler/but-sdk";

export type ChangeDiff = {
	path: string;
	diff: UnifiedDiff | null;
};

export const DIFF_SERVICE = new InjectionToken<DiffService>("DiffService");

type UnitySemanticRequest = {
	active: boolean;
	started: boolean;
	promise: Promise<UnitySemanticDiff | null>;
};

export class DiffService {
	private unitySemanticDiffCache = new Map<string, UnitySemanticRequest>();
	private unitySemanticDiffQueue = Promise.resolve();

	constructor(private backendApi: BackendApi) {}

	getDiff(projectId: string, change: TreeChange) {
		return this.backendApi.endpoints.getDiff.useQuery({ projectId, change });
	}

	getUnitySemanticDiff(projectId: string, change: TreeChange) {
		return this.backendApi.endpoints.unitySemanticDiff.useQuery({ projectId, change });
	}

	peekUnitySemanticDiff(projectId: string, change: TreeChange) {
		return this.unitySemanticDiffCache.get(unitySemanticDiffCacheKey(projectId, change))?.promise;
	}

	fetchUnitySemanticDiff(projectId: string, change: TreeChange, force = false) {
		const key = unitySemanticDiffCacheKey(projectId, change);
		const cached = this.unitySemanticDiffCache.get(key);
		if (cached && !force) {
			cached.active = true;
			return cached.promise;
		}
		if (cached) {
			cached.active = false;
		}

		const request: UnitySemanticRequest = {
			active: true,
			started: false,
			promise: this.unitySemanticDiffQueue.then(async () => {
				if (!request.active) return null;
				request.started = true;
				const result = (await this.backendApi.endpoints.unitySemanticDiff.fetch(
					{ projectId, change },
					{ forceRefetch: force },
				)) as UnitySemanticDiff | null;
				if (!request.active) {
					this.unitySemanticDiffCache.delete(key);
				}
				return result;
			}),
		};
		this.unitySemanticDiffCache.set(key, request);
		this.unitySemanticDiffQueue = request.promise.then(
			() => undefined,
			() => undefined,
		);
		request.promise.catch(() => {
			if (this.unitySemanticDiffCache.get(key)?.promise === request.promise) {
				this.unitySemanticDiffCache.delete(key);
			}
		});
		return request.promise;
	}

	cancelUnitySemanticDiff(projectId: string, change: TreeChange) {
		const key = unitySemanticDiffCacheKey(projectId, change);
		const request = this.unitySemanticDiffCache.get(key);
		if (!request) return;

		request.active = false;
		if (!request.started) {
			this.unitySemanticDiffCache.delete(key);
		}
	}

	getUnitySmartMergePreview(projectId: string, path: string) {
		return this.backendApi.endpoints.unitySmartMergePreview.useQuery({ projectId, path });
	}

	get runUnitySmartMerge() {
		return this.backendApi.endpoints.runUnitySmartMerge.mutate;
	}

	get assignHunk() {
		return this.backendApi.endpoints.assignHunk.mutate;
	}

	async fetchDiff(projectId: string, change: TreeChange) {
		const { getDiff } = this.backendApi.endpoints;
		return await getDiff.fetch({ projectId, change });
	}

	getChanges(projectId: string, changes: TreeChange[]) {
		const args = changes.map((change) => ({ projectId, change }));
		const { getDiff } = this.backendApi.endpoints;
		return getDiff.useQueries(args, {
			transform: (data, args): ChangeDiff => ({ path: args.change.path, diff: data }),
		});
	}

	async fetchChanges(projectId: string, changes: TreeChange[]): Promise<ChangeDiff[]> {
		const args = changes.map((change) => ({ projectId, change }));
		const responses = await Promise.all(
			args.map((arg) =>
				this.backendApi.endpoints.getDiff.fetch(arg, {
					transform: (diff, args) => ({
						path: args.change.path,
						diff,
					}),
				}),
			),
		);
		return responses.filter(isDefined);
	}
}

function unitySemanticDiffCacheKey(projectId: string, change: TreeChange) {
	return JSON.stringify({
		projectId,
		pathBytes: change.pathBytes,
		status: change.status,
	});
}
