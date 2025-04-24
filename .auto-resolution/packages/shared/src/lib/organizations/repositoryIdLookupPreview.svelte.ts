import { getContext } from '$lib/context';
import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import { RepositoryIdLookupService } from '$lib/organizations/repositoryIdLookupService';
import { repositoryIdLookupTable } from '$lib/organizations/repositoryIdLookupsSlice';
import { stringifyProjectIdentity, type LoadableRepositoryId } from '$lib/organizations/types';
import { AppState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';

export function lookupProject(
	owner: string,
	slug: string,
	inView?: InView
): Reactive<LoadableRepositoryId | undefined> {
	const projectLookupService = getContext(RepositoryIdLookupService);
	const appState = getContext(AppState);

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
