import { registerInterest, type InView } from '$lib/interest/registerInterestFunction.svelte';
import { map } from '$lib/network/loadable';
import { organizationTable } from '$lib/organizations/organizationsSlice';
import { projectTable } from '$lib/organizations/projectsSlice';
import type { OrganizationService } from '$lib/organizations/organizationService';
import type { LoadableOrganization, LoadableProject } from '$lib/organizations/types';
import type { AppOrganizationsState, AppProjectsState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';

export function getOrganizations(
	appState: AppOrganizationsState,
	organizationService: OrganizationService,
	inView?: InView
): Reactive<LoadableOrganization[]> {
	registerInterest(organizationService.getOrganizationListingInterest(), inView);
	const current = $derived(organizationTable.selectors.selectAll(appState.organizations));

	return {
		get current() {
			return current;
		}
	};
}

export function getOrganizationBySlug(
	appState: AppOrganizationsState,
	organizationService: OrganizationService,
	slug: string,
	inView?: InView
): Reactive<LoadableOrganization | undefined> {
	registerInterest(organizationService.getOrganizationWithDetailsInterest(slug), inView);
	const current = $derived(organizationTable.selectors.selectById(appState.organizations, slug));

	return {
		get current() {
			return current;
		}
	};
}

export function getOrganizationProjects(
	appState: AppProjectsState & AppOrganizationsState,
	organizationService: OrganizationService,
	slug: string,
	inView?: InView
): Reactive<LoadableProject[] | undefined> {
	registerInterest(organizationService.getOrganizationWithDetailsInterest(slug), inView);
	const organization = $derived(
		organizationTable.selectors.selectById(appState.organizations, slug)
	);
	const projects = $derived(
		map(
			organization,
			(organization) =>
				(organization.projectRepositoryIds || [])
					.map((id) => projectTable.selectors.selectById(appState.projects, id))
					.filter((a) => a !== undefined) as LoadableProject[]
		)
	);

	return {
		get current() {
			return projects;
		}
	};
}
