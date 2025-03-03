import { apiToPatch, type ApiPatchCommit, type PatchCommit } from '$lib/patches/types';
import { apiToPermissions, type ApiPermissions, type Permissions } from '$lib/permissions';
import { apiToUserMaybe, type ApiUserMaybe, type UserMaybe } from '$lib/users/types';
import type { LoadableData } from '$lib/network/types';

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
	contributors: ApiUserMaybe[];
	patches: ApiPatchCommit[];
	repository_id: string;
	branch_stack_id?: string;
	branch_stack_order?: number;
	permissions: ApiPermissions;
	owner_login?: string;
	review_status: string;
	forge_url: string | undefined;
	forge_description: string | undefined;
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
	contributors: UserMaybe[];
	patchCommitIds: string[];
	patches: PatchCommit[];
	reviewStatus: string;
	repositoryId: string;
	stackId: string;
	stackOrder: number;
	permissions: Permissions;
	forgeUrl: string | undefined;
	forgeDescription: string | undefined;
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
		contributors: api.contributors.map(apiToUserMaybe),
		patchCommitIds: api.patches.map((patch) => patch.change_id),
		patches: api.patches.map(apiToPatch),
		reviewStatus: api.review_status,
		repositoryId: api.repository_id,
		// Its good enough
		stackId: api.branch_stack_id || String(Math.random()),
		stackOrder: api.branch_stack_order || 1,
		permissions: apiToPermissions(api.permissions),
		forgeUrl: api.forge_url,
		forgeDescription: api.forge_description
	};
}

export type LoadableBranchUuid = LoadableData<string, string>;
export type LoadableBranchReviewListing = LoadableData<string[], string>;

export function toCombineSlug(ownerSlug: string, projectSlug: string) {
	return `${ownerSlug}/${projectSlug}`;
}
