import type { LayoutLoad } from './$types';

export const prerender = false;

// eslint-disable-next-line
export const load: LayoutLoad = async ({ params }) => {
	return {
		projectId: params.projectId
	};
};
