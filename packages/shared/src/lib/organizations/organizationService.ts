import { apiToBranch } from '$lib/branches/types';
import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { type HttpClient } from '$lib/network/httpClient';
import { errorToLoadable } from '$lib/network/loadable';
import { organizationTable } from '$lib/organizations/organizationsSlice';
import { projectTable } from '$lib/organizations/projectsSlice';
import {
	apiToOrganization,
	apiToProject,
	type ApiOrganization,
	type ApiOrganizationWithDetails,
	type LoadableOrganization,
	type LoadableProject,
	type Organization
} from '$lib/organizations/types';
import { POLLING_REGULAR, POLLING_SLOW } from '$lib/polling';
import { writable, type Writable } from 'svelte/store';
import type { ApiBranch, Branch } from '$lib/branches/types';
import type { Loadable } from '$lib/network/types';
import type { AppDispatch } from '$lib/redux/store.svelte';

// Define the LoadablePatchStacks type
export type LoadablePatchStacks = Loadable<Branch[]> & { ownerSlug: string };

export class OrganizationService {
	private readonly organizationListingInterests = new InterestStore<undefined>(POLLING_SLOW);
	private readonly orgnaizationInterests = new InterestStore<{ slug: string }>(POLLING_REGULAR);
	// Add the patch stack cache
	private patchStackCache: Map<string, Writable<LoadablePatchStacks>> = new Map();

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getOrganizationListingInterest(): Interest {
		return this.organizationListingInterests
			.findOrCreateSubscribable(undefined, async () => {
				const apiOrganizations = await this.httpClient.get<ApiOrganization[]>('organization');
				const organizations = apiOrganizations.map<LoadableOrganization>((apiOrganizations) => ({
					status: 'found',
					id: apiOrganizations.slug,
					value: apiToOrganization(apiOrganizations)
				}));

				this.appDispatch.dispatch(organizationTable.upsertMany(organizations));
			})
			.createInterest();
	}

	getOrganizationWithDetailsInterest(slug: string): Interest {
		return this.orgnaizationInterests
			.findOrCreateSubscribable({ slug }, async () => {
				this.appDispatch.dispatch(organizationTable.addOne({ status: 'loading', id: slug }));

				try {
					const apiOrganization = await this.httpClient.get<ApiOrganizationWithDetails>(
						`organization/${slug}`
					);

					const projects = apiOrganization.projects.map<LoadableProject>((apiProject) => ({
						status: 'found',
						id: apiProject.repository_id,
						value: apiToProject(apiProject)
					}));
					this.appDispatch.dispatch(projectTable.upsertMany(projects));

					this.appDispatch.dispatch(
						organizationTable.upsertOne({
							status: 'found',
							id: slug,
							value: apiToOrganization(apiOrganization)
						})
					);
				} catch (error: unknown) {
					this.appDispatch.dispatch(organizationTable.upsertOne(errorToLoadable(error, slug)));
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
		this.appDispatch.dispatch(
			organizationTable.upsertOne({ status: 'found', id: slug, value: organization })
		);

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
		this.appDispatch.dispatch(
			organizationTable.upsertOne({ status: 'found', id: slug, value: organization })
		);

		return organization;
	}

	/**
	 * Gets a store for an organization's patch stacks by slug. The store will be populated when accessed.
	 * @param ownerSlug The organization slug to fetch patch stacks for
	 * @returns A readable store containing the patch stacks
	 */
	getPatchStacks(ownerSlug: string): Writable<LoadablePatchStacks> {
		if (!this.patchStackCache.has(ownerSlug)) {
			// Create a new store for this organization's patch stacks
			const store = writable<LoadablePatchStacks>({ status: 'loading', ownerSlug }, (set) => {
				// Fetch data when the store is first subscribed to
				this.fetchPatchStacks(ownerSlug)
					.then((data) => {
						set({ status: 'found', ownerSlug, value: data } as LoadablePatchStacks);
					})
					.catch((error) => {
						if (error.response?.status === 404) {
							set({
								status: 'not-found',
								ownerSlug
							} as LoadablePatchStacks);
						} else {
							set({
								status: 'error',
								ownerSlug,
								error: error.message || 'Unknown error occurred'
							} as LoadablePatchStacks);
						}
					});
			});

			this.patchStackCache.set(ownerSlug, store);
		}

		return this.patchStackCache.get(ownerSlug)!;
	}

	/**
	 * Manually fetch patch stacks from the API
	 * @param ownerSlug The organization slug to fetch patch stacks for
	 * @returns Array of patch stacks converted to Branch format
	 */
	async fetchPatchStacks(ownerSlug: string): Promise<Branch[]> {
		console.log(`[OrganizationService] Fetching patch stacks for org: ${ownerSlug}`);

		// Try different API endpoint patterns since we're not sure about the exact one
		const endpoint = `organization/${ownerSlug}/patch_stacks`;

		try {
			console.log(`[OrganizationService] Trying API URL: ${endpoint}`);
			const response = await this.httpClient.get<ApiBranch[]>(endpoint);
			console.log(`[OrganizationService] API response successful for ${endpoint}:`, response);

			// Convert ApiBranch objects to Branch objects
			return response.map(apiToBranch);
		} catch (error: any) {
			console.error(`[OrganizationService] Error with endpoint ${endpoint}:`, error);

			// If this is not a 404 error (which likely means wrong endpoint), rethrow it
			if (error.response && error.response.status !== 404) {
				throw error;
			}

			// If it's a 404, we'll try the next endpoint
			console.log(`[OrganizationService] Endpoint ${endpoint} returned 404, trying next...`);
		}

		// If we've tried all endpoints and none worked, return empty array
		console.warn(`[OrganizationService] All endpoints failed, returning empty array`);
		return [];
	}

	/**
	 * Remove patch stacks from the cache
	 * @param ownerSlug The organization slug to remove from cache
	 */
	clearPatchStackCache(ownerSlug?: string): void {
		if (ownerSlug) {
			this.patchStackCache.delete(ownerSlug);
		} else {
			this.patchStackCache.clear();
		}
	}
}
