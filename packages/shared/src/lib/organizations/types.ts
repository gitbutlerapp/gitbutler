import type { LoadableData } from '$lib/network/types';

export type ApiProject = {
	slug: string;
	owner: string;
	parent_project?: ApiProject;
	name: string;
	description: string;

	repository_id: string;
	code_repository_id: string;
	git_url: string;
	code_git_url: string;

	created_at: string;
	updated_at: string;
};

export type Project = {
	/** Repository ID is the main identifier for a cloud project */
	repositoryId: string;

	slug: string;
	owner: string;
	parentProjectRepositoryId?: string;
	name: string;
	description: string;

	codeRepositoryId: string;
	gitUrl: string;
	codeGitUrl: string;

	createdAt: string;
	updatedAt: string;
};

export type LoadableProject = LoadableData<Project, Project['repositoryId']>;

export function apiToProject(apiProject: ApiProject): Project {
	return {
		repositoryId: apiProject.repository_id,
		slug: apiProject.slug,
		owner: apiProject.owner,
		parentProjectRepositoryId: apiProject.parent_project?.repository_id,
		name: apiProject.name,
		description: apiProject.description,
		codeRepositoryId: apiProject.code_repository_id,
		gitUrl: apiProject.git_url,
		codeGitUrl: apiProject.code_git_url,
		createdAt: apiProject.created_at,
		updatedAt: apiProject.updated_at
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
