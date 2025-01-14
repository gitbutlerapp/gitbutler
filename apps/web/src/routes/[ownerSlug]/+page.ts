import type { OwnerParameters } from '$lib/project/types';
import type { PageLoad } from './$types';

// eslint-disable-next-line func-style
export const load: PageLoad<OwnerParameters> = async ({ params }) => {
	return {
		ownerSlug: params.ownerSlug
	};
};
