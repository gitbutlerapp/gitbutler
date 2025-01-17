import type { ProjectParameters } from '$lib/routing';
import type { PageLoad } from './$types';

// eslint-disable-next-line func-style
export const load: PageLoad<ProjectParameters> = async ({ params }) => {
	return {
		ownerSlug: params.ownerSlug,
		projectSlug: params.projectSlug
	};
};
