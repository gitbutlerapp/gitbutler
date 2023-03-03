import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ parent, params }) => {
	const { projects } = await parent();
	return {
		project: projects.get(params.projectId)
	};
};
