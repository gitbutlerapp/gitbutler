import { apiToPermissions, type ApiPermissions, type Permissions } from '$lib/permissions';
import type { LoadableData } from '$lib/network/types';

export type ApiProject = {
	slug: string;
	owner: string;
	owner_type: string;
	parent_project?: ApiProject;
	name: string;
	description: string;
	readme?: string;

	active_reviews_count: number;

	repository_id: string;
	code_repository_id: string;
	git_url: string;
	code_git_url: string;

	permissions: ApiPermissions;

	created_at: string;
	updated_at: string;
	last_pushed_at?: string;
};

export type Project = {
	/** Repository ID is the main identifier for a cloud project */
	repositoryId: string;

	slug: string;
	owner: string;
	ownerType: string;

	parentProject?: Project;
	parentProjectRepositoryId?: string;

	activeReviewsCount?: number;
	name: string;
	description: string;
	readme?: string;

	codeRepositoryId: string;
	gitUrl: string;
	codeGitUrl: string;

	permissions: Permissions;

	createdAt: string;
	updatedAt: string;
	lastPushedAt?: string;
};

export type LoadableProject = LoadableData<Project, Project['repositoryId']>;

export function apiToProject(apiProject: ApiProject): Project {
	return {
		repositoryId: apiProject.repository_id,
		slug: apiProject.slug,
		owner: apiProject.owner,
		ownerType: apiProject.owner_type,
		parentProject: apiProject.parent_project ? apiToProject(apiProject.parent_project) : undefined,
		parentProjectRepositoryId: apiProject.parent_project?.repository_id,
		activeReviewsCount: apiProject.active_reviews_count,
		name: apiProject.name,
		description: apiProject.description,
		codeRepositoryId: apiProject.code_repository_id,
		gitUrl: apiProject.git_url,
		codeGitUrl: apiProject.code_git_url,
		permissions: apiToPermissions(apiProject.permissions),
		createdAt: apiProject.created_at,
		updatedAt: apiProject.updated_at,
		readme: apiProject.readme,
		lastPushedAt: apiProject.last_pushed_at
	};
}

export type ApiOrganizationUser = {
	slug: string;
	login: string;
};

export type ApiOrganization = {
	slug: string;
	name: string;
	description: string;
	created_at: string;
	invite_code?: string;
};

export type ApiOrganizationWithDetails = ApiOrganization & {
	projects: ApiProject[];
	members: ApiOrganizationUser[];
};

export type Organization = {
	slug: string;
	name?: string;
	description?: string;
	createdAt: string;
	/** only present if you are the organization admin */
	inviteCode?: string;

	memberLogins?: string[];
	projectRepositoryIds?: string[];
};

export type LoadableOrganization = LoadableData<Organization, Organization['slug']>;

export function apiToOrganization(
	apiOrganization: ApiOrganization | ApiOrganizationWithDetails
): Organization {
	return {
		slug: apiOrganization.slug,
		name: apiOrganization.name,
		description: apiOrganization.description,
		createdAt: apiOrganization.created_at,
		inviteCode: apiOrganization.invite_code,
		projectRepositoryIds:
			'projects' in apiOrganization
				? apiOrganization.projects.map(({ repository_id }) => repository_id)
				: undefined,
		memberLogins:
			'members' in apiOrganization ? apiOrganization.members.map(({ login }) => login) : undefined
	};
}

export function stringifyProjectIdentity(owner: string, slug: string): string {
	return `${owner}/${slug}`;
}

export type LoadableRepositoryId = LoadableData<string, string>;
