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

/*
	ALL optional
	expose :owner_login
	expose :oplog_sha
	expose :description
	expose :reviewers, using: Butler::API::Entities::UserSimple
	expose :repository_id
	expose :branch_stack_id
	expose :branch_stack_order
	expose :permissions, using: Butler::API::Entities::Permission
	expose :patches do |status, options|
		Butler::API::Entities::Patch.represent(status.patches.order(position: :asc), options)
	end
	expose :forge_description
	expose :forge_url
	expose :created_at
*/

export type ApiBranch = {
	branch_id: string;
	oplog_sha?: string;
	uuid: string;
	title?: string;
	description?: string;
	status?: BranchStatus;
	version?: number;
	created_at?: string;
	updated_at: string;
	stack_size?: number;
	contributors: ApiUserMaybe[];
	patches?: ApiPatchCommit[];
	repository_id?: string;
	branch_stack_id?: string;
	branch_stack_order?: number;
	permissions: ApiPermissions;
	owner_login?: string;
	review_status: string;
	forge_url?: string | undefined;
	forge_description?: string | undefined;
	review_url: string | undefined;
	project_full_slug: string;
};

export type Branch = {
	branchId: string;
	oplogSha?: string;
	uuid: string;
	title?: string;
	description?: string;
	status?: BranchStatus;
	version?: number;
	createdAt?: string;
	updatedAt: string;
	stackSize?: number;
	contributors: UserMaybe[];
	patchCommitIds: string[];
	patches?: PatchCommit[];
	reviewStatus: string;
	repositoryId?: string;
	stackId: string;
	stackOrder: number;
	permissions: Permissions;
	forgeUrl?: string | undefined;
	forgeDescription?: string | undefined;
	reviewUrl: string | undefined;
	projectFullSlug: string;
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
		patchCommitIds: api.patches?.map((patch) => patch.change_id) || [],
		patches: api.patches?.map((api) => apiToPatch(api)) || [],
		reviewStatus: api.review_status,
		repositoryId: api.repository_id,
		// Its good enough
		stackId: api.branch_stack_id || String(Math.random()),
		stackOrder: api.branch_stack_order || 1,
		permissions: apiToPermissions(api.permissions),
		forgeUrl: api.forge_url,
		forgeDescription: api.forge_description,
		reviewUrl: api.review_url,
		projectFullSlug: api.project_full_slug
	};
}

export type LoadableBranchUuid = LoadableData<string, string>;
export type LoadableBranchReviewListing = LoadableData<string[], string>;

export function toCombineSlug(ownerSlug: string, projectSlug: string) {
	return `${ownerSlug}/${projectSlug}`;
}

export function branchReviewListingKey(
	ownerSlug: string,
	projectSlug: string,
	branchStatus: BranchStatus
) {
	return `${ownerSlug}/${projectSlug}:${branchStatus}`;
}
