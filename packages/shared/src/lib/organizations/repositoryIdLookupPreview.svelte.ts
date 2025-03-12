import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import { repositoryIdLookupTable } from '$lib/organizations/repositoryIdLookupsSlice';
import { stringifyProjectIdentity, type LoadableRepositoryId } from '$lib/organizations/types';
import type { RepositoryIdLookupService } from '$lib/organizations/repositoryIdLookupService';
import type { AppRepositoryIdLookupsState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';

export function lookupProject(
	appState: AppRepositoryIdLookupsState,
	projectLookupService: RepositoryIdLookupService,
	owner: string,
	slug: string,
	inView?: InView
): Reactive<LoadableRepositoryId | undefined> {
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
