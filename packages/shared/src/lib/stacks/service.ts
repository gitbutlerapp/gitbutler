import { derived, writable, type Readable } from 'svelte/store';
import type { HttpClient } from '$lib/httpClient';

interface ApiPatchStatstics {
	file_count: number;
	section_count: number;
	lines: number;
	deletions: number;
	files: string[];
}

class PatchStatstics {
	fileCount: number;
	sectionCount: number;
	lines: number;
	deletions: number;
	files: string[];

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

class PatchReview {
	viewed: boolean;
	signedOff: boolean;
	rejected: boolean;

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

// eslint-disable-next-line @typescript-eslint/no-unused-vars
class Patch {
	changeId: string;
	commitSha: string;
	title?: string;
	description?: string;
	position: number;
	version: number;
	contributors: string[];
	statistics: PatchStatstics;
	review: PatchReview;
	reviewAll: PatchReview[];

	constructor(apiPatch: ApiPatch) {
		this.changeId = apiPatch.change_id;
		this.commitSha = apiPatch.commit_sha;
		this.title = apiPatch.title;
		this.description = apiPatch.description;
		this.position = apiPatch.position || 0;
		this.version = apiPatch.version || 0;
		this.contributors = apiPatch.contributors;
		this.statistics = new PatchStatstics(apiPatch.statistics);
		this.review = new PatchReview(apiPatch.review);
		this.reviewAll = apiPatch.review_all.map((review) => new PatchReview(review));
	}
}

const enum PatchStackStatus {
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
	status?: PatchStackStatus;
	version?: number;
	created_at: string;
	stack_size?: number;
	contributors: string[];
	patches: ApiPatch[];
}

class PatchStack {
	branchId: string;
	oplogSha?: string;
	uuid: string;
	title?: string;
	description?: string;
	status?: PatchStackStatus;
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

interface PatchStackCreationParams {
	branch_id: string;
	oplog_sha: string;
}

interface PatchStackUpdateParams {
	status: 'active' | 'closed';
	title: string;
	description: string;
}

export class PatchStacksApiService {
	constructor(
		private readonly repositoryId: string,
		private readonly httpClient: HttpClient
	) {}

	async getPatchStacks(status: PatchStackStatus = PatchStackStatus.All): Promise<ApiPatchStack[]> {
		// TODO(CTO): Support optional filtering query params `branch_id` and `status`
		return await this.httpClient.get<ApiPatchStack[]>(
			`/patch_stack/${this.repositoryId}?status=${status}`
		);
	}

	async createPatchStack(params: PatchStackCreationParams): Promise<ApiPatchStack> {
		return await this.httpClient.post<ApiPatchStack>(`/patch_stack/${this.repositoryId}`, {
			body: params
		});
	}

	async updatePatchStack(
		patchStackUuid: string,
		params: PatchStackUpdateParams
	): Promise<ApiPatchStack> {
		return await this.httpClient.put<ApiPatchStack>(`/patch_stack/${patchStackUuid}`, {
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
export class PatchStacksService {
	#apiPatchStacks = writable<ApiPatchStack[]>([], (set) => {
		let canceled = false;

		const callback = (() => {
			this.patchStacksApiService.getPatchStacks().then((patchStacks) => {
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

	#patchStacks = derived(this.#apiPatchStacks, (apiPatchStacks) => {
		return apiPatchStacks.map((apiPatchStack) => new PatchStack(apiPatchStack));
	});

	constructor(
		private readonly _repositoryId: string,
		private readonly patchStacksApiService: PatchStacksApiService
	) {}

	/** An unordered list of patch stacks for a given repository */
	get patchStacks(): Readable<PatchStack[]> {
		return this.#patchStacks;
	}

	/** Refresh the list of patch stacks */
	async refresh(): Promise<void> {
		const patchStacks = await this.patchStacksApiService.getPatchStacks();
		this.#apiPatchStacks.set(patchStacks);
	}
}
