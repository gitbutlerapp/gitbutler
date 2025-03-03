import {
	apiToUserMaybe,
	apiToUserSimple,
	type ApiUserMaybe,
	type ApiUserSimple,
	type UserMaybe,
	type UserSimple
} from '$lib/users/types';
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
	viewed: ApiUserSimple[];
	signed_off: ApiUserSimple[];
	rejected: ApiUserSimple[];
};

export type PatchReview = {
	viewed: UserSimple[];
	signedOff: UserSimple[];
	rejected: UserSimple[];
};

export function apiToPatchReview(api: ApiPatchReview): PatchReview {
	return {
		viewed: api.viewed.map(apiToUserSimple),
		signedOff: api.signed_off.map(apiToUserSimple),
		rejected: api.rejected.map(apiToUserSimple)
	};
}

type PatchTypes = 'PatchCommit' | 'PatchIdable';

export type ApiBasePatch = {
	type: PatchTypes;
	statistics: ApiPatchStatistics;
	sections: ApiSection[] | undefined;
	created_at: string;
	updated_at: string;
};

export type ApiPatchCommit = ApiBasePatch & {
	type: 'PatchCommit';
	change_id: string;
	commit_sha: string;
	// patch_sha: string; Not sure this is real
	title: string | undefined;
	description: string | undefined;
	position: number | undefined;
	version: number | undefined;
	comment_count: number;
	contributors: ApiUserMaybe[];
	review: ApiPatchReview;
	review_all: ApiPatchReview;
	review_status: string;
	sections?: ApiSection[];
	created_at: string;
	updated_at: string;
};

export type ApiPatchIdable = ApiBasePatch & {
	type: 'PatchIdable';
	patch_id: string;
};

export type ApiPatch = ApiPatchCommit | ApiPatchIdable;

export type BasePatch = {
	type: PatchTypes;
	// patch_sha: string; Not sure this is real
	statistics: PatchStatistics;
	sectionIds: number[] | undefined;
	createdAt: string;
	updatedAt: string;
};

export type PatchCommit = BasePatch & {
	type: 'PatchCommit';
	changeId: string;
	commitSha: string;
	// patch_sha: string; Not sure this is real
	title: string | undefined;
	description: string | undefined;
	position: number | undefined;
	version: number | undefined;
	commentCount: number;
	contributors: UserMaybe[];
	review: PatchReview;
	reviewAll: PatchReview;
	reviewStatus: string;
	sectionIds?: number[];
	createdAt: string;
	updatedAt: string;
};

export type PatchIdable = BasePatch & {
	type: 'PatchIdable';
	patchId: string;
};

export type Patch = PatchCommit | PatchIdable;

export function getPatchStatus(
	patch: PatchCommit
): 'approved' | 'changes-requested' | 'unreviewed' | 'in-discussion' {
	if (patch.reviewAll.rejected.length > 0) return 'changes-requested';
	if (patch.reviewAll.signedOff.length > 0) return 'approved';
	if (patch.commentCount > 0) return 'in-discussion';
	return 'unreviewed';
}

export type LoadablePatchCommit = LoadableData<PatchCommit, PatchCommit['changeId']>;
export type LoadablePatchIdable = LoadableData<PatchIdable, PatchIdable['patchId']>;

export function apiToPatch(api: ApiPatchCommit): PatchCommit;
export function apiToPatch(api: ApiPatchIdable): PatchIdable;
export function apiToPatch(api: ApiPatch): Patch {
	if (api.type === 'PatchCommit') {
		return {
			type: api.type,
			changeId: api.change_id,
			commitSha: api.commit_sha,
			title: api.title,
			description: api.description,
			position: api.position,
			version: api.version,
			commentCount: api.comment_count || 0,
			contributors: api.contributors.map(apiToUserMaybe),
			statistics: apiToPatchStatistics(api.statistics),
			review: apiToPatchReview(api.review),
			reviewAll: apiToPatchReview(api.review_all),
			reviewStatus: api.review_status,
			sectionIds: api.sections?.map((section) => section.id),
			createdAt: api.created_at,
			updatedAt: api.updated_at
		};
	} else if (api.type === 'PatchIdable') {
		return {
			type: api.type,
			statistics: apiToPatchStatistics(api.statistics),
			sectionIds: api.sections?.map((section) => section.id),
			createdAt: api.created_at,
			updatedAt: api.updated_at,
			patchId: api.patch_id
		} as PatchIdable;
	} else {
		throw new Error('Unreachable');
	}
}
