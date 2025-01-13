import type { PageLoad } from './$types';
import type { OwnerParameters } from './types';

// eslint-disable-next-line func-style
export const load: PageLoad<OwnerParameters> = async ({ params }) => {
	return {
		ownerSlug: params.ownerSlug
	};
};
