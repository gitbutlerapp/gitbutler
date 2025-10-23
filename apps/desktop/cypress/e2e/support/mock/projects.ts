import type { Project } from '$lib/project/project';

export const PROJECT_ID = '1';

export const MOCK_PROJECT_A: Project = {
	id: PROJECT_ID,
	title: 'Project A',
	description: 'Description for Project A',
	path: '/path/to/projectA',
	api: undefined,
	preferred_key: 'systemExecutable',
	ok_with_force_push: true,
	force_push_protection: false,
	omit_certificate_check: false,
	use_diff_context: true,
	is_open: false,
	forge_override: undefined,
	preferred_forge_user: null,
	gerrit_mode: false
};

export function createMockProject(id: string, title: string, path: string): Project {
	return {
		...MOCK_PROJECT_A,
		id,
		title,
		path
	};
}

export function listProjects() {
	return [MOCK_PROJECT_A];
}

type GetProjectArgs = {
	projectId: string;
};

export function isGetProjectArgs(args: unknown): args is GetProjectArgs {
	return (
		typeof args === 'object' &&
		args !== null &&
		'projectId' in args &&
		typeof args.projectId === 'string'
	);
}

export function getProject(args: GetProjectArgs) {
	return MOCK_PROJECT_A.id === args.projectId ? MOCK_PROJECT_A : undefined;
}

type AddProjectArgs = {
	path: string;
};

export function isAddProjectArgs(args: unknown): args is AddProjectArgs {
	return (
		typeof args === 'object' && args !== null && 'path' in args && typeof args.path === 'string'
	);
}
