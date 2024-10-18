import { writableDerived } from '$lib/storeUtils';
import { derived, get, type Readable, type Writable } from 'svelte/store';
import type { HttpClient } from '$lib/httpClient';

interface ApiPatchStatstics {
	file_count: number;
	section_count: number;
	lines: number;
	deletions: number;
	files: string[];
}

export class CloudPatchStatsitics {
	readonly fileCount: number;
	readonly sectionCount: number;
	readonly lines: number;
	readonly deletions: number;
	readonly files: string[];

	constructor(apiPatchStatstics: ApiPatchStatstics) {
		this.fileCount = apiPatchStatstics.file_count;
		this.sectionCount = apiPatchStatstics.section_count;
		this.lines = apiPatchStatstics.lines;
		this.deletions = apiPatchStatstics.deletions;
		this.files = apiPatchStatstics.files;
	}
}

interface ApiPatchReview {
	viewed: boolean;
	signed_off: boolean;
	rejected: boolean;
}

export class CloudPatchReview {
	readonly viewed: boolean;
	readonly signedOff: boolean;
	readonly rejected: boolean;

	constructor(apiPatchReview: ApiPatchReview) {
		this.viewed = apiPatchReview.viewed;
		this.signedOff = apiPatchReview.signed_off;
		this.rejected = apiPatchReview.rejected;
	}
}

interface ApiPatch {
	change_id: string;
	commit_sha: string;
	// patch_sha: string; Not sure this is real
	title?: string;
	description?: string;
	position?: number;
	version?: number;
	contributors: string[];
	statistics: ApiPatchStatstics;
	review: ApiPatchReview;
	review_all: ApiPatchReview;
}

export class CloudPatch {
	changeId: string;
	commitSha: string;
	title?: string;
	description?: string;
	position: number;
	version: number;
	contributors: string[];
	statistics: CloudPatchStatsitics;
	review: CloudPatchReview;
	reviewAll: CloudPatchReview;

	constructor(apiPatch: ApiPatch) {
		this.changeId = apiPatch.change_id;
		this.commitSha = apiPatch.commit_sha;
		this.title = apiPatch.title;
		this.description = apiPatch.description;
		this.position = apiPatch.position || 0;
		this.version = apiPatch.version || 0;
		this.contributors = apiPatch.contributors;
		this.statistics = new CloudPatchStatsitics(apiPatch.statistics);
		this.review = new CloudPatchReview(apiPatch.review);
		this.reviewAll = new CloudPatchReview(apiPatch.review_all);
	}
}

export const enum CloudPatchStackStatus {
	Active = 'active',
	Inactive = 'inactive',
	Closed = 'closed',
	Loading = 'loading',
	All = 'all'
}

interface ApiPatchStack {
	branch_id: string;
	oplog_sha?: string;
	uuid: string;
	title?: string;
	description?: string;
	status?: CloudPatchStackStatus;
	version?: number;
	created_at: string;
	stack_size?: number;
	contributors: string[];
	patches: ApiPatch[];
}

export class CloudPatchStack {
	branchId: string;
	oplogSha?: string;
	uuid: string;
	title?: string;
	description?: string;
	status?: CloudPatchStackStatus;
	version: number;
	createdAt: string;
	stackSize: number;
	contributors: string[];
	// TODO(CTO): Determine the best way to talk about these nested objects.
	//              Should they be in their own reactive service?
	patches: CloudPatch[];

	constructor(apiPatchStack: ApiPatchStack) {
		this.branchId = apiPatchStack.branch_id;
		this.oplogSha = apiPatchStack.oplog_sha;
		this.uuid = apiPatchStack.uuid;
		this.title = apiPatchStack.title;
		this.description = apiPatchStack.description;
		this.status = apiPatchStack.status;
		this.version = apiPatchStack.version || 0;
		this.createdAt = apiPatchStack.created_at;
		this.stackSize = apiPatchStack.stack_size || 0;
		this.contributors = apiPatchStack.contributors;
		this.patches = apiPatchStack.patches?.map((patch) => new CloudPatch(patch));
	}
}

export interface PatchStackCreationParams {
	branch_id: string;
	oplog_sha: string;
}

export interface PatchStackUpdateParams {
	status: 'active' | 'closed';
	title: string;
	description: string;
}

export class PatchStacksApiService {
	readonly canGetPatchStacks: Readable<boolean>;
	readonly canCreatePatchStack: Readable<boolean>;

	constructor(private readonly httpClient: HttpClient) {
		this.canGetPatchStacks = httpClient.authenticationAvailable;
		this.canCreatePatchStack = httpClient.authenticationAvailable;
	}

	async getPatchStacks(
		repositoryId: string,
		status: CloudPatchStackStatus = CloudPatchStackStatus.All
	): Promise<ApiPatchStack[] | undefined> {
		// TODO(CTO): Support optional filtering query param `branch_id`
		try {
			return await this.httpClient.get<ApiPatchStack[]>(
				`patch_stack/${repositoryId}?status=${status}`
			);
		} catch (e) {
			// If the internet is down, silently fail
			if (e instanceof TypeError) {
				return undefined;
			} else {
				throw e;
			}
		}
	}

	async createPatchStack(
		repositoryId: string,
		params: PatchStackCreationParams
	): Promise<ApiPatchStack> {
		return await this.httpClient.post<ApiPatchStack>(`patch_stack`, {
			body: {
				...params,
				project_id: repositoryId
			}
		});
	}

	async updatePatchStack(
		patchStackUuid: string,
		params: PatchStackUpdateParams
	): Promise<ApiPatchStack> {
		return await this.httpClient.put<ApiPatchStack>(`patch_stack/${patchStackUuid}`, {
			body: params
		});
	}
}

const MINUTES_15 = 15 * 60 * 1000;

type LoadableOptional<T> =
	| {
			state: 'found';
			value: T;
	  }
	| {
			state: 'uninitialized' | 'not-found';
	  };

/**
 * Provides a list of patch stacks for a given repository.
 *
 * The list of patch stacks is kept up-to-date automatically, whenever
 * operations on a patch stack have been performed, or every 15 minutes.
 */
export class CloudPatchStacksService {
	/** Whether a patch stack can be created given the current internal state of the patch stack service */
	canCreatePatchStack: Readable<boolean>;

	#apiPatchStacks: Writable<ApiPatchStack[] | undefined>;

	/** An unordered list of patch stacks for a given repository */
	readonly patchStacks: Readable<CloudPatchStack[] | undefined>;

	constructor(
		readonly repositoryId: Readable<string | undefined>,
		private readonly patchStacksApiService: PatchStacksApiService
	) {
		const values = derived(
			[this.patchStacksApiService.canGetPatchStacks, this.repositoryId],
			(values) => values
		);

		this.#apiPatchStacks = writableDerived<
			ApiPatchStack[] | undefined,
			[boolean, string | undefined]
		>(values, undefined, ([canGetPatchStacks, repositoryId], set) => {
			if (!repositoryId || !canGetPatchStacks) {
				set(undefined);
				return;
			}

			let canceled = false;

			const callback = (() => {
				this.patchStacksApiService.getPatchStacks(repositoryId).then((patchStacks) => {
					if (!canceled) set(patchStacks);
				});
			}).bind(this);

			// Automatically refresh every 15 minutes
			callback();
			const interval = setInterval(callback, MINUTES_15);

			return () => {
				canceled = true;
				clearInterval(interval);
			};
		});

		this.patchStacks = derived(this.#apiPatchStacks, (apiPatchStacks) => {
			return apiPatchStacks?.map((apiPatchStack) => new CloudPatchStack(apiPatchStack));
		});

		this.canCreatePatchStack = derived(
			[this.repositoryId, this.patchStacksApiService.canCreatePatchStack],
			([repositoryId, canCreatePatchStack]) => !!repositoryId && !!canCreatePatchStack
		);
	}

	async createPatchStack(branchId: string, oplogSha: string): Promise<CloudPatchStack> {
		const repositoryId = get(this.repositoryId);

		// Repository ID will be defined
		if (!this.canCreatePatchStack) {
			throw new Error('Can not create a patch stack');
		}

		const apiPatchStack = await this.patchStacksApiService.createPatchStack(repositoryId!, {
			branch_id: branchId,
			oplog_sha: oplogSha
		});

		// TODO(CTO): Determine whether updating like this is preferable to
		// doing a full refresh.
		// A full refresh will ensure consistency, but will be more expensive.
		this.#apiPatchStacks.update((apiPatchStacks) => [...(apiPatchStacks || []), apiPatchStack]);

		return new CloudPatchStack(apiPatchStack);
	}

	/** Refresh the list of patch stacks */
	async refresh(): Promise<void> {
		const repositoryId = get(this.repositoryId);
		const canGetPatchStacks = get(this.patchStacksApiService.canGetPatchStacks);

		if (repositoryId && canGetPatchStacks) {
			const patchStacks = await this.patchStacksApiService.getPatchStacks(repositoryId);
			this.#apiPatchStacks.set(patchStacks);
		} else {
			this.#apiPatchStacks.set(undefined);
		}
	}

	#patchStacksByBranchIds = new Map<string, Readable<LoadableOptional<CloudPatchStack>>>();
	patchStackForBranchId(branchId: string): Readable<LoadableOptional<CloudPatchStack>> {
		let store = this.#patchStacksByBranchIds.get(branchId);
		if (store) return store;

		store = derived(this.patchStacks, (patchStacks): LoadableOptional<CloudPatchStack> => {
			if (!patchStacks) return { state: 'uninitialized' };
			const patchStack = patchStacks.find((patchStack) => patchStack.branchId === branchId);
			if (patchStack) {
				return { state: 'found', value: patchStack };
			} else {
				return { state: 'not-found' };
			}
		});
		this.#patchStacksByBranchIds.set(branchId, store);
		return store;
	}

	#patchStacksByIds = new Map<string, Readable<LoadableOptional<CloudPatchStack>>>();
	patchStackForId(patchStackId: string): Readable<LoadableOptional<CloudPatchStack>> {
		let store = this.#patchStacksByIds.get(patchStackId);
		if (store) return store;

		store = derived(this.patchStacks, (patchStacks): LoadableOptional<CloudPatchStack> => {
			if (!patchStacks) return { state: 'uninitialized' };
			const patchStack = patchStacks.find((patchStack) => patchStack.uuid === patchStackId);
			if (patchStack) {
				return { state: 'found', value: patchStack };
			} else {
				return { state: 'not-found' };
			}
		});
		this.#patchStacksByIds.set(patchStackId, store);
		return store;
	}
}
