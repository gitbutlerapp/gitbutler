import { apiToPermissions, type ApiPermissions, type Permissions } from '$lib/permissions';
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

export type ApiPatchReviewUser = {
	id: number;
	avatar_url: string | null;
	email: string | null;
	login: string | null;
	name: string | null;
};

export type PatchReviewUser = {
	id: number;
	avatarUrl: string | undefined;
	email: string | undefined;
	login: string | undefined;
	name: string | undefined;
};

export function apiToPatchReviewUser(api: ApiPatchReviewUser): PatchReviewUser {
	return {
		id: api.id,
		avatarUrl: api.avatar_url ?? undefined,
		email: api.email ?? undefined,
		login: api.login ?? undefined,
		name: api.name ?? undefined
	};
}

export type ApiPatchReview = {
	viewed: ApiPatchReviewUser[];
	signed_off: ApiPatchReviewUser[];
	rejected: ApiPatchReviewUser[];
};

export type PatchReview = {
	viewed: PatchReviewUser[];
	signedOff: PatchReviewUser[];
	rejected: PatchReviewUser[];
};

export function apiToPatchReview(api: ApiPatchReview): PatchReview {
	return {
		viewed: api.viewed.map(apiToPatchReviewUser),
		signedOff: api.signed_off.map(apiToPatchReviewUser),
		rejected: api.rejected.map(apiToPatchReviewUser)
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

async function getUsersWithAvatarsFromMails(userEmails: string[]) {
	return await Promise.all(
		userEmails.map(async (user) => {
			return {
				srcUrl: await gravatarUrlFromEmail(user),
				name: user
			};
		})
	);
}

export type Commenter = {
	avatarUrl?: string;
	email?: string;
	login?: string;
	name?: string;
};

export async function getUsersWithAvatars(commenters: Commenter[]) {
	return await Promise.all(
		commenters.map(async (commenter) => {
			const name = commenter.login ?? commenter.email ?? commenter.name ?? 'unknown';
			const email = commenter.email ?? 'unknown';
			return {
				srcUrl: commenter.avatarUrl ?? (await gravatarUrlFromEmail(email)),
				name
			};
		})
	);
}

export async function getPatchContributorsWithAvatars(patch: Patch) {
	return await getUsersWithAvatarsFromMails(patch.contributors);
}

export async function getPatchApproversWithAvatars(patch: Patch) {
	return await getUsersWithAvatars(patch.review.signedOff);
}

export async function getPatchRejectorsWithAvatars(patch: Patch) {
	return await getUsersWithAvatars(patch.review.rejected);
}

export async function getPatchViewersWithAvatars(patch: Patch) {
	return await getUsersWithAvatars(patch.review.viewed);
}

export async function getPatchViewersAllWithAvatars(patch: Patch) {
	return await getUsersWithAvatars(patch.reviewAll.viewed);
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
	permissions: ApiPermissions;
	owner_login?: string;
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
	patches: Patch[];
	repositoryId: string;
	stackId: string;
	stackOrder: number;
	permissions: Permissions;
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
		patches: api.patches.map(apiToPatch),
		repositoryId: api.repository_id,
		// Its good enough
		stackId: api.branch_stack_id || String(Math.random()),
		stackOrder: api.branch_stack_order || 1,
		permissions: apiToPermissions(api.permissions)
	};
}

export type LoadableBranchUuid = LoadableData<string, string>;
export type LoadableBranchReviewListing = LoadableData<string[], string>;

export function toCombineSlug(ownerSlug: string, projectSlug: string) {
	return `${ownerSlug}/${projectSlug}`;
}
