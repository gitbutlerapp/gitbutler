import { InjectionToken } from '@gitbutler/core/context';
import { writable, type Writable } from 'svelte/store';
import type {
	OwnerResponse,
	LoadableOwner,
	ExtendedUser,
	ExtendedOrganization
} from '$lib/owner/types';
import type { HttpClient } from '@gitbutler/shared/network/httpClient';

export const OWNER_SERVICE = new InjectionToken<OwnerService>('OwnerService');

/**
 * Service for fetching information about owners (users and organizations)
 */
export class OwnerService {
	private ownerCache: Map<string, Writable<LoadableOwner>> = new Map();

	constructor(private readonly httpClient: HttpClient) {}

	getOwner(slug: string): Writable<LoadableOwner> {
		if (!this.ownerCache.has(slug)) {
			// Create a new store for this owner
			const store = writable<LoadableOwner>({ status: 'loading', slug }, (set) => {
				// Fetch data when the store is first subscribed to
				this.fetchOwner(slug)
					.then((data) => {
						set({ status: 'found', slug, value: data });
					})
					.catch((error) => {
						if (error.response?.status === 404) {
							set({
								status: 'not-found',
								slug,
								value: { type: 'not_found' }
							});
						} else {
							set({
								status: 'error',
								slug,
								error: error.message || 'Unknown error occurred'
							});
						}
					});
			});

			this.ownerCache.set(slug, store);
		}

		return this.ownerCache.get(slug)!;
	}

	async fetchOwner(slug: string): Promise<OwnerResponse> {
		try {
			const response = await this.httpClient.get<any>(`/api/owners/${slug}`);

			// Determine type based on the response's owner_type
			if (response.owner_type === 'organization') {
				const org: ExtendedOrganization = {
					slug: response.slug,
					name: response.name || slug,
					description: response.description,
					createdAt: response.created_at,
					avatarUrl: response.avatar_url,
					projects: response.projects,
					inviteCode: response.invite_code,
					members: response.members
				};

				return {
					type: 'organization',
					data: org
				};
			} else if (response.owner_type === 'user') {
				const user: ExtendedUser = {
					id: response.id,
					name: response.name || response.login,
					login: response.login,
					email: response.email,
					avatarUrl: response.avatar_url,
					readme: response.readme,
					website: response.website,
					twitter: response.twitter,
					bluesky: response.bluesky,
					timezone: response.timezone,
					location: response.location,
					organizations: response.organizations
				};

				return {
					type: 'user',
					data: user
				};
			}

			// If owner_type is not set or unknown, return not found
			return { type: 'not_found' };
		} catch (error: any) {
			if (error.response) {
				// Return not_found for 404s
				if (error.response.status === 404) {
					return { type: 'not_found' };
				}

				// For all other error status codes, throw the error
				throw new Error(`Error ${error.response.status}: ${error.message}`);
			}

			// For network errors or other non-HTTP errors
			throw error;
		}
	}
}
