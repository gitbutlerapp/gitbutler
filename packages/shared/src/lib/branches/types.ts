import { deduplicate } from '$lib/utils/array';
import { gravatarUrlFromEmail } from '@gitbutler/ui/avatar/gravatar';
import type { LoadableData } from '$lib/network/types';

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

export type ApiPatchStatistics = {
	file_count: number;
	section_count: number;
	lines: number;
	deletions: number;
	files: string[];
};

export type PatchStatistics = {
	fileCount: number;
	sectionCount: number;
	lines: number;
	deletions: number;
	files: string[];
};

export function apiToPatchStatistics(api: ApiPatchStatistics): PatchStatistics {
	return {
		fileCount: api.file_count,
		sectionCount: api.section_count,
		lines: api.lines,
		deletions: api.deletions,
		files: api.files
	};
}

export type ApiPatchReview = {
	viewed: string[];
	signed_off: string[];
	rejected: string[];
};

export type PatchReview = {
	viewed: string[];
	signedOff: string[];
	rejected: string[];
};

export function apiToPatchReview(api: ApiPatchReview): PatchReview {
	return {
		viewed: api.viewed,
		signedOff: api.signed_off,
		rejected: api.rejected
	};
}

export type ApiPatch = {
	change_id: string;
	commit_sha: string;
	// patch_sha: string; Not sure this is real
	title?: string;
	description?: string;
	position?: number;
	version?: number;
	contributors: string[];
	statistics: ApiPatchStatistics;
	review: ApiPatchReview;
	review_all: ApiPatchReview;
	sections?: ApiSection[];
	created_at: string;
	updated_at: string;
};

export type Patch = {
	changeId: string;
	commitSha: string;
	// patch_sha: string; Not sure this is real
	title?: string;
	description?: string;
	position?: number;
	version?: number;
	contributors: string[];
	statistics: PatchStatistics;
	review: PatchReview;
	reviewAll: PatchReview;
	sectionIds?: number[];
	createdAt: string;
	updatedAt: string;
};

export function getPatchStatus(
	patch: Patch
): 'approved' | 'changes-requested' | 'unreviewed' | 'in-discussion' {
	if (patch.review.rejected.length > 0) return 'changes-requested';
	if (patch.review.signedOff.length > 0) return 'approved';
	if (patch.review.viewed.length > 0) return 'in-discussion';
	return 'unreviewed';
}

async function getUsersWithAvatars(userEmails: string[]) {
	return await Promise.all(
		userEmails.map(async (user) => {
			return {
				srcUrl: await gravatarUrlFromEmail(user),
				name: user
			};
		})
	);
}

export async function getPatchContributorsWithAvatars(patch: Patch) {
	return await getUsersWithAvatars(patch.contributors);
}

export async function getPatchReviewersWithAvatars(patch: Patch) {
	const reviewers = deduplicate([...patch.review.rejected, ...patch.review.signedOff]);
	return await getUsersWithAvatars(reviewers);
}

export async function getPatchViewersWithAvatars(patch: Patch) {
	return await getUsersWithAvatars(patch.review.viewed);
}

export type LoadablePatch = LoadableData<Patch, Patch['changeId']>;

export function apiToPatch(api: ApiPatch): Patch {
	return {
		changeId: api.change_id,
		commitSha: api.commit_sha,
		title: api.title,
		description: api.description,
		position: api.position,
		version: api.version,
		contributors: api.contributors,
		statistics: apiToPatchStatistics(api.statistics),
		review: apiToPatchReview(api.review),
		reviewAll: apiToPatchReview(api.review_all),
		sectionIds: api.sections?.map((section) => section.id),
		createdAt: api.created_at,
		updatedAt: api.updated_at
	};
}

export enum BranchStatus {
	Active = 'active',
	Inactive = 'inactive',
	Closed = 'closed',
	Loading = 'loading',
	All = 'all',
	Previous = 'previous'
}

export type ApiBranch = {
	branch_id: string;
	oplog_sha?: string;
	uuid: string;
	title?: string;
	description?: string;
	status?: BranchStatus;
	version?: number;
	created_at: string;
	updated_at: string;
	stack_size?: number;
	contributors: string[];
	patches: ApiPatch[];
	repository_id: string;
	branch_stack_id?: string;
	branch_stack_order?: number;
};

export type Branch = {
	branchId: string;
	oplogSha?: string;
	uuid: string;
	title?: string;
	description?: string;
	status?: BranchStatus;
	version?: number;
	createdAt: string;
	updatedAt: string;
	stackSize?: number;
	contributors: string[];
	patchIds: string[];
	repositoryId: string;
	stackId: string;
	stackOrder: number;
};

export type LoadableBranch = LoadableData<Branch, Branch['uuid']>;

export function apiToBranch(api: ApiBranch): Branch {
	return {
		branchId: api.branch_id,
		oplogSha: api.oplog_sha,
		uuid: api.uuid,
		title: api.title,
		description: api.description,
		status: api.status,
		version: api.version,
		createdAt: api.created_at,
		updatedAt: api.updated_at,
		stackSize: api.stack_size,
		contributors: api.contributors,
		patchIds: api.patches.map((patch) => patch.change_id),
		repositoryId: api.repository_id,
		// Its good enough
		stackId: api.branch_stack_id || String(Math.random()),
		stackOrder: api.branch_stack_order || 1
	};
}

export type LoadableBranchUuid = LoadableData<string, string>;
