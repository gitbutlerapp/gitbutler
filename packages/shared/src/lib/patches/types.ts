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

export type ApiPatch = {
	change_id: string;
	commit_sha: string;
	// patch_sha: string; Not sure this is real
	title?: string;
	description?: string;
	position?: number;
	version?: number;
	comment_count?: number;
	contributors: ApiUserMaybe[];
	statistics: ApiPatchStatistics;
	review: ApiPatchReview;
	review_all: ApiPatchReview;
	sections?: ApiSection[];
	created_at: string;
	updated_at: string;
	previous_version_sha: string | undefined;
};

export type Patch = {
	changeId: string;
	commitSha: string;
	// patch_sha: string; Not sure this is real
	title?: string;
	description?: string;
	position?: number;
	version?: number;
	commentCount: number;
	contributors: UserMaybe[];
	statistics: PatchStatistics;
	review: PatchReview;
	reviewAll: PatchReview;
	sectionIds?: number[];
	createdAt: string;
	updatedAt: string;
	previousVersionSha: string | undefined;
};

export function getPatchStatus(
	patch: Patch
): 'approved' | 'changes-requested' | 'unreviewed' | 'in-discussion' {
	if (patch.reviewAll.rejected.length > 0) return 'changes-requested';
	if (patch.reviewAll.signedOff.length > 0) return 'approved';
	if (patch.commentCount > 0) return 'in-discussion';
	return 'unreviewed';
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
		commentCount: api.comment_count || 0,
		contributors: api.contributors.map(apiToUserMaybe),
		statistics: apiToPatchStatistics(api.statistics),
		review: apiToPatchReview(api.review),
		reviewAll: apiToPatchReview(api.review_all),
		sectionIds: api.sections?.map((section) => section.id),
		createdAt: api.created_at,
		updatedAt: api.updated_at,
		previousVersionSha: api.previous_version_sha
	};
}
