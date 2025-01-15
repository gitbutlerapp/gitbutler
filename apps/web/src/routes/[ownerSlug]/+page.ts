import type { OwnerParameters } from '$lib/routing';
import type { PageLoad } from './$types';

// eslint-disable-next-line func-style
export const load: PageLoad<OwnerParameters> = async ({ params }) => {
	return {
		ownerSlug: params.ownerSlug
	};
};
