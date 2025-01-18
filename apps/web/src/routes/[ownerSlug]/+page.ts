import type { PageLoad } from './$types';
import type { OwnerParameters } from '@gitbutler/shared/routing/webRoutes';

// eslint-disable-next-line func-style
export const load: PageLoad<OwnerParameters> = async ({ params }) => {
	return {
		ownerSlug: params.ownerSlug
	};
};
