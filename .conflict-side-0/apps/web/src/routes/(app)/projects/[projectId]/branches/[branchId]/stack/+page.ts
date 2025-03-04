import type { PageLoad } from './$types';

// eslint-disable-next-line func-style
export const load: PageLoad = ({ params }) => {
	return {
		projectId: params.projectId,
		branchId: params.branchId
	};
};
