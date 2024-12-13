import { InterestStore, type Interest } from '$lib/interest/intrestStore';
import { type HttpClient } from '$lib/network/httpClient';
import { ApiError } from '$lib/network/types';
import {
	addOrganization,
	upsertOrganization,
	upsertOrganizations
} from '$lib/organizations/organizationsSlice';
import { upsertProjects } from '$lib/organizations/projectsSlice';
import {
	apiToOrganization,
	apiToProject,
	type ApiOrganization,
	type ApiOrganizationWithDetails,
	type LoadableOrganization,
	type Organization
} from '$lib/organizations/types';
import { POLLING_REGULAR, POLLING_SLOW } from '$lib/polling';
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
				const organizations = apiOrganizations.map(
					(apiOrganizations) =>
						({
							type: 'found',
							id: apiOrganizations.slug,
							value: apiToOrganization(apiOrganizations)
						}) as LoadableOrganization
				);

				this.appDispatch.dispatch(upsertOrganizations(organizations));
			})
			.createInterest();
	}

	getOrganizationWithDetailsInterest(slug: string): Interest {
		return this.orgnaizationInterests
			.findOrCreateSubscribable({ slug }, async () => {
				this.appDispatch.dispatch(addOrganization({ type: 'loading', id: slug }));

				try {
					const apiOrganization = await this.httpClient.get<ApiOrganizationWithDetails>(
						`organization/${slug}`
					);
					const organization = apiToOrganization(apiOrganization);
					const projects = apiOrganization.projects.map(apiToProject);

					this.appDispatch.dispatch(
						upsertOrganization({ type: 'found', id: slug, value: organization })
					);
					this.appDispatch.dispatch(upsertProjects(projects));
				} catch (error: unknown) {
					if (error instanceof ApiError && error.response.status === 404) {
						this.appDispatch.dispatch(upsertOrganization({ type: 'not-found', id: slug }));
					} else if (error instanceof Error) {
						this.appDispatch.dispatch(upsertOrganization({ type: 'error', id: slug, error }));
					}
				}
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
		const organization = apiToOrganization(apiOrganization);
		this.appDispatch.dispatch(upsertOrganization({ type: 'found', id: slug, value: organization }));

		return organization;
	}

	async joinOrganization(slug: string, joinCode: string) {
		const apiOrganization = await this.httpClient.post<ApiOrganizationWithDetails>(
			`organization/${slug}/join`,
			{
				body: { invite_code: joinCode }
			}
		);

		const organization = apiToOrganization(apiOrganization);
		this.appDispatch.dispatch(upsertOrganization({ type: 'found', id: slug, value: organization }));

		return organization;
	}
}
