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

export type DiffSection = {
	id: number;
	sectionType: 'diff';
	identifier: string;
	title?: string;
	position?: number;

	diffSha: string;
	baseFileSha: string;
	newFileSha: string;
	oldPath?: string;
	oldSize?: number;
	newPath?: string;
	newSize?: number;
	hunks?: number;
	lines?: number;
	deletions?: number;
	diffPatch?: string;
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

export type TextSection = {
	id: number;
	sectionType: 'text';
	identifier: string;
	title?: string;
	position?: number;

	version?: number;
	type?: string;
	code?: string;
	plainText?: string;
	data?: unknown;
};

export type ApiSection = ApiDiffSection | ApiTextSection;
export type Section = DiffSection | TextSection;

export function apiToSection(apiSection: ApiSection): Section {
	if (apiSection.section_type === 'diff') {
		return {
			id: apiSection.id,
			sectionType: 'diff',
			identifier: apiSection.identifier,
			title: apiSection.title,
			position: apiSection.position,
			diffSha: apiSection.diff_sha,
			baseFileSha: apiSection.base_file_sha,
			newFileSha: apiSection.new_file_sha,
			oldPath: apiSection.old_path,
			oldSize: apiSection.old_size,
			newPath: apiSection.new_path,
			newSize: apiSection.new_size,
			hunks: apiSection.hunks,
			lines: apiSection.lines,
			deletions: apiSection.deletions,
			diffPatch: apiSection.diff_patch
		};
	} else {
		return {
			id: apiSection.id,
			sectionType: 'text',
			identifier: apiSection.identifier,
			title: apiSection.title,
			position: apiSection.position,
			version: apiSection.version,
			type: apiSection.type,
			code: apiSection.code,
			plainText: apiSection.plain_text,
			data: apiSection.data
		};
	}
}

export type ApiCommitReview = {
	change_id: string;
	commit_sha: string;
	patch_sha: string;
	title: string;
	description: string;
	position: number;
	version: number;
	created_at: string;
	contributors: string[];
	statistics: {
		file_count: number;
		section_count: number;
		lines: number;
		deletions: number;
	};
	review: {
		viewed: string[];
		signed_off: string[];
		rejected: string[];
	};
	review_all: {
		viewed: string[];
		signed_off: string[];
		rejected: string[];
	};
	sections?: ApiSection[];
};

export type CommitReview = {
	changeId: string;
	commitSha: string;
	patchSha: string;
	title: string;
	description: string;
	position: number;
	version: number;
	createdAt: string;
	contributors: string[];
	statistics: {
		fileCount: number;
		sectionCount: number;
		lines: number;
		deletions: number;
	};
	review: {
		viewed: string[];
		signedOff: string[];
		rejected: string[];
	};
	reviewAll: {
		viewed: string[];
		signedOff: string[];
		rejected: string[];
	};
	sectionIds?: number[];
};

export function apiToCommitReview(apiCommitReview: ApiCommitReview): CommitReview {
	return {
		changeId: apiCommitReview.change_id,
		commitSha: apiCommitReview.commit_sha,
		patchSha: apiCommitReview.patch_sha,
		title: apiCommitReview.title,
		description: apiCommitReview.description,
		position: apiCommitReview.position,
		version: apiCommitReview.version,
		createdAt: apiCommitReview.created_at,
		contributors: apiCommitReview.contributors,
		statistics: {
			fileCount: apiCommitReview.statistics.file_count,
			sectionCount: apiCommitReview.statistics.section_count,
			lines: apiCommitReview.statistics.lines,
			deletions: apiCommitReview.statistics.deletions
		},
		review: {
			viewed: apiCommitReview.review.viewed,
			signedOff: apiCommitReview.review.signed_off,
			rejected: apiCommitReview.review.rejected
		},
		reviewAll: {
			viewed: apiCommitReview.review_all.viewed,
			signedOff: apiCommitReview.review_all.signed_off,
			rejected: apiCommitReview.review_all.rejected
		},
		sectionIds: apiCommitReview.sections?.map((section) => section.id)
	};
}

export const enum BranchReviewStatus {
	Active = 'active',
	Inactive = 'inactive',
	Closed = 'closed',
	Loading = 'loading',
	All = 'all'
}

export type ApiBranchReview = {
	branch_id: string;
	oplog_sha: string;
	uuid: string;
	title: string;
	description?: string;
	status: BranchReviewStatus;
	version: number;
	created_at: string;
	stack_size: number;
	contributors: string[];
	patches: ApiCommitReview[];
};

export type BranchReview = {
	branchId: string;
	branchReviewId: string;
	title: string;
	description?: string;
	status: BranchReviewStatus;
	version: number;
	createdAt: string;
	stackSize: number;
	contributors: string[];
	commitsReviewChangeIds: string[];
};

export function apiToBranchReview(apiBranchReview: ApiBranchReview): BranchReview {
	return {
		branchId: apiBranchReview.branch_id,
		branchReviewId: apiBranchReview.uuid,
		title: apiBranchReview.title,
		description: apiBranchReview.description,
		status: apiBranchReview.status,
		version: apiBranchReview.version,
		createdAt: apiBranchReview.created_at,
		stackSize: apiBranchReview.stack_size,
		contributors: apiBranchReview.contributors,
		commitsReviewChangeIds: apiBranchReview.patches.map((patch) => patch.change_id)
	};
}
