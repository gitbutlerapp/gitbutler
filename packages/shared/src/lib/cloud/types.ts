export interface ApiPatchStatstics {
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

export interface ApiPatchReview {
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

export interface ApiPatch {
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

export interface ApiBranch {
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

export interface ApiRepository {
	name: string;
	description: string | null;
	repository_id: string;
	git_url: string;
	created_at: string;
	updated_at: string;
}

export class CloudRepository {
	readonly name: string;
	readonly description: string | null;
	readonly repositoryId: string;
	readonly gitUrl: string;
	readonly createdAt: Date;
	readonly updatedAt: Date;

	constructor(apiRepository: ApiRepository) {
		this.name = apiRepository.name;
		this.description = apiRepository.description;
		this.repositoryId = apiRepository.repository_id;
		this.gitUrl = apiRepository.git_url;
		this.createdAt = new Date(apiRepository.created_at);
		this.updatedAt = new Date(apiRepository.updated_at);
	}
}
