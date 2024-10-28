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

export const enum CloudBranchStatus {
	Active = 'active',
	Inactive = 'inactive',
	Closed = 'closed',
	Loading = 'loading',
	All = 'all'
}

interface ApiBranch {
	branch_id: string;
	oplog_sha?: string;
	uuid: string;
	title?: string;
	description?: string;
	status?: CloudBranchStatus;
	version?: number;
	created_at: string;
	stack_size?: number;
	contributors: string[];
	patches: ApiPatch[];
}

export class CloudBranch {
	branchId: string;
	oplogSha?: string;
	uuid: string;
	title?: string;
	description?: string;
	status?: CloudBranchStatus;
	version: number;
	createdAt: string;
	stackSize: number;
	contributors: string[];
	// TODO(CTO): Determine the best way to talk about these nested objects.
	//              Should they be in their own reactive service?
	patches: CloudPatch[];

	constructor(apiBranch: ApiBranch) {
		this.branchId = apiBranch.branch_id;
		this.oplogSha = apiBranch.oplog_sha;
		this.uuid = apiBranch.uuid;
		this.title = apiBranch.title;
		this.description = apiBranch.description;
		this.status = apiBranch.status;
		this.version = apiBranch.version || 0;
		this.createdAt = apiBranch.created_at;
		this.stackSize = apiBranch.stack_size || 0;
		this.contributors = apiBranch.contributors;
		this.patches = apiBranch.patches?.map((patch) => new CloudPatch(patch));
	}
}

export interface BranchCreationParams {
	branch_id: string;
	oplog_sha: string;
}

export interface BranchUpdateParams {
	status: 'active' | 'closed';
	title: string;
	description: string;
}

export class BranchesApiService {
	readonly canGetBranches: Readable<boolean>;
	readonly canCreateBranch: Readable<boolean>;

	constructor(private readonly httpClient: HttpClient) {
		this.canGetBranches = httpClient.authenticationAvailable;
		this.canCreateBranch = httpClient.authenticationAvailable;
	}

	async getBranches(
		repositoryId: string,
		status: CloudBranchStatus = CloudBranchStatus.All
	): Promise<ApiBranch[] | undefined> {
		// TODO(CTO): Support optional filtering query param `branch_id`
		try {
			return await this.httpClient.get<ApiBranch[]>(`patch_stack/${repositoryId}?status=${status}`);
		} catch (e) {
			// If the internet is down, silently fail
			if (e instanceof TypeError) {
				return undefined;
			} else {
				throw e;
			}
		}
	}

	async createBranch(repositoryId: string, params: BranchCreationParams): Promise<ApiBranch> {
		return await this.httpClient.post<ApiBranch>(`patch_stack`, {
			body: {
				...params,
				project_id: repositoryId
			}
		});
	}

	async updateBranch(cloudBranchUuid: string, params: BranchUpdateParams): Promise<ApiBranch> {
		return await this.httpClient.put<ApiBranch>(`patch_stack/${cloudBranchUuid}`, {
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
export class CloudBranchesService {
	/** Whether a patch stack can be created given the current internal state of the patch stack service */
	canCreateBranch: Readable<boolean>;

	#apiBranches: Writable<ApiBranch[] | undefined>;

	/** An unordered list of patch stacks for a given repository */
	readonly branches: Readable<CloudBranch[] | undefined>;

	constructor(
		readonly repositoryId: Readable<string | undefined>,
		private readonly branchesApiService: BranchesApiService
	) {
		const values = derived(
			[this.branchesApiService.canGetBranches, this.repositoryId],
			(values) => values
		);

		this.#apiBranches = writableDerived<ApiBranch[] | undefined, [boolean, string | undefined]>(
			values,
			undefined,
			([canGetBranches, repositoryId], set) => {
				if (!repositoryId || !canGetBranches) {
					set(undefined);
					return;
				}

				let canceled = false;

				const callback = (() => {
					this.branchesApiService.getBranches(repositoryId).then((cloudBranches) => {
						if (!canceled) set(cloudBranches);
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

		this.branches = derived(this.#apiBranches, (apiBranches) => {
			return apiBranches?.map((apiBranch) => new CloudBranch(apiBranch));
		});

		this.canCreateBranch = derived(
			[this.repositoryId, this.branchesApiService.canCreateBranch],
			([repositoryId, canCreateBranch]) => !!repositoryId && !!canCreateBranch
		);
	}

	async createBranch(branchId: string, oplogSha: string): Promise<CloudBranch> {
		const repositoryId = get(this.repositoryId);

		// Repository ID will be defined
		if (!this.canCreateBranch) {
			throw new Error('Can not create a patch stack');
		}

		const apiBranch = await this.branchesApiService.createBranch(repositoryId!, {
			branch_id: branchId,
			oplog_sha: oplogSha
		});

		// TODO(CTO): Determine whether updating like this is preferable to
		// doing a full refresh.
		// A full refresh will ensure consistency, but will be more expensive.
		this.#apiBranches.update((apiBranches) => [...(apiBranches || []), apiBranch]);

		return new CloudBranch(apiBranch);
	}

	/** Refresh the list of patch stacks */
	async refresh(): Promise<void> {
		const repositoryId = get(this.repositoryId);
		const canGetBranches = get(this.branchesApiService.canGetBranches);

		if (repositoryId && canGetBranches) {
			const branches = await this.branchesApiService.getBranches(repositoryId);
			this.#apiBranches.set(branches);
		} else {
			this.#apiBranches.set(undefined);
		}
	}

	#branchesByBranchIds = new Map<string, Readable<LoadableOptional<CloudBranch>>>();
	/** Finds a cloud branch for a given client branch ID */
	branchForBranchId(branchId: string): Readable<LoadableOptional<CloudBranch>> {
		let store = this.#branchesByBranchIds.get(branchId);
		if (store) return store;

		store = derived(this.branches, (branches): LoadableOptional<CloudBranch> => {
			if (!branches) return { state: 'uninitialized' };
			const branch = branches.find((cloudBranch) => cloudBranch.branchId === branchId);
			if (branch) {
				return { state: 'found', value: branch };
			} else {
				return { state: 'not-found' };
			}
		});
		this.#branchesByBranchIds.set(branchId, store);
		return store;
	}

	#branchesByIds = new Map<string, Readable<LoadableOptional<CloudBranch>>>();
	branchForId(cloudBranchId: string): Readable<LoadableOptional<CloudBranch>> {
		let store = this.#branchesByIds.get(cloudBranchId);
		if (store) return store;

		store = derived(this.branches, (branches): LoadableOptional<CloudBranch> => {
			if (!branches) return { state: 'uninitialized' };
			const branch = branches.find((cloudBranch) => cloudBranch.uuid === cloudBranchId);
			if (branch) {
				return { state: 'found', value: branch };
			} else {
				return { state: 'not-found' };
			}
		});
		this.#branchesByIds.set(cloudBranchId, store);
		return store;
	}
}
