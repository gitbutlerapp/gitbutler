import { registerInterest } from '$lib/interest/registerInterestFunction.svelte';
import { organizationsSelectors } from '$lib/organizations/organizationsSlice';
import type { OrganizationService } from '$lib/organizations/organizationService';
import type { LoadableOrganization } from '$lib/organizations/types';
import type { AppOrganizationsState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';

export function getOrganizations(
	appState: AppOrganizationsState,
	organizationService: OrganizationService
): Reactive<LoadableOrganization[]> {
	registerInterest(organizationService.getOrganizationListingInterest());
	const current = $derived(organizationsSelectors.selectAll(appState.organizations));

	return {
		get current() {
			return current;
		}
	};
}

export function getOrganizationBySlug(
	appState: AppOrganizationsState,
	organizationService: OrganizationService,
	slug: string
): Reactive<LoadableOrganization | undefined> {
	registerInterest(organizationService.getOrganizationWithDetailsInterest(slug));
	const current = $derived(organizationsSelectors.selectById(appState.organizations, slug));

	return {
		get current() {
			return current;
		}
	};
}
