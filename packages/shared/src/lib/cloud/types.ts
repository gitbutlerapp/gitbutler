export const MINUTES_15 = 15 * 60 * 1000;

export type LoadableOptional<T> =
	| {
			state: 'found';
			value: T;
	  }
	| {
			state: 'uninitialized' | 'not-found';
			value?: undefined;
	  };

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
	viewed: string[];
	signed_off: string[];
	rejected: string[];
}

/** Lists of emails of people who have viewed or reviewed */
export class CloudPatchReview {
	readonly viewed: string[];
	readonly signedOff: string[];
	readonly rejected: string[];

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

export type ApiDiffSection = {
	id: number;
	section_type: 'diff';
	identifier: string;
	title?: string;
	position?: number;

	diff_sha: string;
	base_file_sha: string;
	new_file_sha: string;
	old_path?: string;
	old_size?: number;
	new_path?: string;
	new_size?: number;
	hunks?: number;
	lines?: number;
	deletions?: number;
	diff_patch?: string;
};

export type ApiTextSection = {
	id: number;
	section_type: 'text';
	identifier: string;
	title?: string;
	position?: number;

	version?: number;
	type?: string;
	code?: string;
	plain_text?: string;
	data?: unknown;
};

export type ApiSection = ApiDiffSection | ApiTextSection;

export interface ApiPatchWithFiles extends ApiPatch {
	sections: ApiSection[];
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

interface CloudSection {
	id: number;
	sectionType: string;
	identifier: string;
	title?: string;
	position?: number;
}

export class CloudTextSection implements CloudSection {
	id: number;
	sectionType: 'text' = 'text' as const;
	identifier: string;
	title?: string | undefined;
	position?: number | undefined;

	version?: number;
	type?: string;
	code?: string;
	plainText?: string;
	data?: unknown;

	constructor(apiTextSection: ApiTextSection) {
		this.id = apiTextSection.id;
		this.identifier = apiTextSection.identifier;
		this.title = apiTextSection.title;
		this.position = apiTextSection.position;
		this.version = apiTextSection.version;
		this.type = apiTextSection.type;
		this.code = apiTextSection.code;
		this.plainText = apiTextSection.plain_text;
		this.data = apiTextSection.data;
	}
}
export class CloudDiffSection implements CloudSection {
	id: number;
	sectionType: 'diff' = 'diff' as const;
	identifier: string;
	title?: string | undefined;
	position?: number | undefined;

	diffSha: string;
	baseFileSha: string;
	newFileSha: string;
	oldPath?: string;
	oldSize?: number;
	newPath?: string;
	newSize?: number;
	hunks: number;
	lines?: number;
	deletions?: number;
	diffPatch?: string;

	constructor(apiDiffSection: ApiDiffSection) {
		this.id = apiDiffSection.id;
		this.identifier = apiDiffSection.identifier;
		this.title = apiDiffSection.title;
		this.position = apiDiffSection.position;

		this.diffSha = apiDiffSection.diff_sha;
		this.baseFileSha = apiDiffSection.base_file_sha;
		this.newFileSha = apiDiffSection.new_file_sha;
		this.oldPath = apiDiffSection.old_path;
		this.oldSize = apiDiffSection.old_size;
		this.newPath = apiDiffSection.new_path;
		this.newSize = apiDiffSection.new_size;
		this.hunks = apiDiffSection.hunks || 0;
		this.lines = apiDiffSection.lines;
		this.deletions = apiDiffSection.deletions;
		this.diffPatch = apiDiffSection.diff_patch;
	}
}

export class CloudPatchWithFiles extends CloudPatch {
	sections: (CloudDiffSection | CloudTextSection)[];

	constructor(apiPatchWithFiles: ApiPatchWithFiles) {
		super(apiPatchWithFiles);

		this.sections = apiPatchWithFiles.sections.map((section) => {
			if (section.section_type === 'diff') {
				return new CloudDiffSection(section);
			} else if (section.section_type === 'text') {
				return new CloudTextSection(section);
			} else {
				// In case the api gets updated
				throw new Error(`Encountered unexpected section type ${(section as any).section_type}`);
			}
		});
	}

	foo() {
		console.log('moo');
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
