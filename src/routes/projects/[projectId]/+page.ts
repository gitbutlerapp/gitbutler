import { building } from '$app/environment';
import { readable } from 'svelte/store';
import type { PageLoad } from './$types';

export const load: PageLoad = async () => {
  const user = building
  ? {
      ...readable<undefined>(undefined),
      set: () => {
        throw new Error('not implemented');
      },
      delete: () => {
        throw new Error('not implemented');
      }
    }
  : await (await import('$lib/users')).default();
	return {
		user
	};
};
