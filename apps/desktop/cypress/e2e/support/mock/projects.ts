export const MOCK_PROJECT_A = {
	id: '1',
	title: 'Project A',
	description: 'Description for Project A',
	path: '/path/to/projectA',
	api: undefined,
	preferred_key: 'systemExecutable',
	ok_with_force_push: true,
	omit_certificate_check: false,
	use_diff_context: true,
	snapshot_lines_threshold: 5,
	is_open: false
};

export function listProjects() {
	return [MOCK_PROJECT_A];
}

type GetProjectArgs = {
	id: string;
};

export function isGetProjectArgs(args: unknown): args is GetProjectArgs {
	return typeof args === 'object' && args !== null && 'id' in args && typeof args.id === 'string';
}

export function getProject(args: GetProjectArgs) {
	return MOCK_PROJECT_A.id === args.id ? MOCK_PROJECT_A : undefined;
}
