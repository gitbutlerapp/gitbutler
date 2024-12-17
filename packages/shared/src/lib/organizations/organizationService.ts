import { InterestStore, type Interest } from '$lib/interest/intrestStore';
import { upsertOrganization, upsertOrganizations } from '$lib/organizations/organizationsSlice';
import { upsertProjects } from '$lib/organizations/projectsSlice';
import {
	apiToOrganization,
	apiToProject,
	type ApiOrganization,
	type ApiOrganizationWithDetails,
	type Organization
} from '$lib/organizations/types';
import { POLLING_REGULAR, POLLING_SLOW } from '$lib/polling';
import type { HttpClient } from '$lib/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

export class OrganizationService {
	private readonly organizationListingInterests = new InterestStore<undefined>(POLLING_SLOW);
	private readonly orgnaizationInterests = new InterestStore<{ slug: string }>(POLLING_REGULAR);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getOrganizationListingInterest(): Interest {
		return this.organizationListingInterests
			.findOrCreateSubscribable(undefined, async () => {
				const apiOrganizations = await this.httpClient.get<ApiOrganization[]>('organization');
				const organizations = apiOrganizations.map(apiToOrganization);

				this.appDispatch.dispatch(upsertOrganizations(organizations));
			})
			.createInterest();
	}

	getOrganizationWithDetailsInterest(slug: string): Interest {
		return this.orgnaizationInterests
			.findOrCreateSubscribable({ slug }, async () => {
				const apiOrganization = await this.httpClient.get<ApiOrganizationWithDetails>(
					`organization/${slug}`
				);
				const organization = apiToOrganization(apiOrganization);
				const projects = apiOrganization.projects.map(apiToProject);

				this.appDispatch.dispatch(upsertOrganization(organization));
				this.appDispatch.dispatch(upsertProjects(projects));
			})
			.createInterest();
	}

	async createOrganization(
		slug: string,
		name?: string,
		description?: string
	): Promise<Organization> {
		const apiOrganization = await this.httpClient.post<ApiOrganizationWithDetails>('organization', {
			body: {
				slug,
				name,
				description
			}
		});
		const orgnaization = apiToOrganization(apiOrganization);
		this.appDispatch.dispatch(upsertOrganization(orgnaization));

		return orgnaization;
	}

	async joinOrganization(slug: string, joinCode: string) {
		const apiOrganization = await this.httpClient.post<ApiOrganizationWithDetails>(
			`organization/${slug}/join`,
			{
				body: { invite_code: joinCode }
			}
		);

		const orgnaization = apiToOrganization(apiOrganization);
		this.appDispatch.dispatch(upsertOrganization(orgnaization));

		return orgnaization;
	}
}
