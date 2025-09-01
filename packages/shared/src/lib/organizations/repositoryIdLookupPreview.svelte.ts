import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import { REPOSITORY_ID_LOOKUP_SERVICE } from '$lib/organizations/repositoryIdLookupService';
import { repositoryIdLookupTable } from '$lib/organizations/repositoryIdLookupsSlice';
import { stringifyProjectIdentity, type LoadableRepositoryId } from '$lib/organizations/types';
import { APP_STATE } from '$lib/redux/store.svelte';
import { inject } from '@gitbutler/core/context';
import type { Reactive } from '$lib/storeUtils';

export function lookupProject(
	owner: string,
	slug: string,
	inView?: InView
): Reactive<LoadableRepositoryId | undefined> {
	const projectLookupService = inject(REPOSITORY_ID_LOOKUP_SERVICE);
	const appState = inject(APP_STATE);

	registerInterest(projectLookupService.getRepositoryIdInterest(owner, slug), inView);
	const repositoryId = $derived(
		repositoryIdLookupTable.selectors.selectById(
			appState.repositoryIdLookups,
			stringifyProjectIdentity(owner, slug)
		)
	);

	return {
		get current() {
			return repositoryId;
		}
	};
}
