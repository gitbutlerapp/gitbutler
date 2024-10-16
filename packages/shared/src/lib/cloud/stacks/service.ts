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
	review_all: ApiPatchReview[];
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
	reviewAll: CloudPatchReview[];

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
		this.reviewAll = apiPatch.review_all.map((review) => new CloudPatchReview(review));
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
	// patches: Patch[];

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
		// this.patches = apiPatchStack.patches?.map((patch) => new Patch(patch));
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
	constructor(private readonly httpClient: HttpClient) {}

	async getPatchStacks(
		repositoryId: string,
		status: CloudPatchStackStatus = CloudPatchStackStatus.All
	): Promise<ApiPatchStack[]> {
		// TODO(CTO): Support optional filtering query param `branch_id`
		return await this.httpClient.get<ApiPatchStack[]>(
			`patch_stack/${repositoryId}?status=${status}`
		);
	}

	async createPatchStack(
		repositoryId: string,
		params: PatchStackCreationParams
	): Promise<ApiPatchStack> {
		return await this.httpClient.post<ApiPatchStack>(`patch_stack/${repositoryId}`, {
			body: params
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

/**
 * Provides a list of patch stacks for a given repository.
 *
 * The list of patch stacks is kept up-to-date automatically, whenever
 * operations on a patch stack have been performed, or every 15 minutes.
 */
export class CloudPatchStacksService {
	#apiPatchStacks: Writable<ApiPatchStack[]>;
	#patchStacks;

	constructor(
		private readonly repositoryId: Readable<string | undefined>,
		private readonly patchStacksApiService: PatchStacksApiService
	) {
		this.#apiPatchStacks = writableDerived<ApiPatchStack[], string | undefined>(
			this.repositoryId,
			[],
			(repositoryId, set) => {
				if (!repositoryId) {
					set([]);
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
			}
		);

		this.#patchStacks = derived(this.#apiPatchStacks, (apiPatchStacks) => {
			return apiPatchStacks.map((apiPatchStack) => new CloudPatchStack(apiPatchStack));
		});
	}

	/** An unordered list of patch stacks for a given repository */
	get patchStacks(): Readable<CloudPatchStack[]> {
		return this.#patchStacks;
	}

	/** Refresh the list of patch stacks */
	async refresh(): Promise<void> {
		const repositoryId = get(this.repositoryId);
		if (!repositoryId) {
			this.#apiPatchStacks.set([]);
			return;
		}
		const patchStacks = await this.patchStacksApiService.getPatchStacks(repositoryId);
		this.#apiPatchStacks.set(patchStacks);
	}
}
