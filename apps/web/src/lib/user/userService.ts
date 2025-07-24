import { setSentryUser } from '$lib/analytics/sentry';
import { apiToBranch } from '@gitbutler/shared/branches/types';
import { InjectionToken } from '@gitbutler/shared/context';
import { get, writable, type Writable } from 'svelte/store';
import type { ApiBranch, Branch } from '@gitbutler/shared/branches/types';
import type { HttpClient } from '@gitbutler/shared/network/httpClient';
import type { Loadable } from '@gitbutler/shared/network/types';

export interface User {
	id: number;
	login: string | undefined;
	avatar_url: string | undefined;
	name: string;
	email: string;
	created_at: Date;
	picture: string;
	supporter: boolean;
	website: string;
	twitter: string;
	bluesky: string;
	timezone: string;
	location: string;
	emailShare: boolean;
	ssh_key_token?: string;
}

// Define the LoadablePatchStacks type using the shared Loadable type
export type LoadablePatchStacks = Loadable<Branch[]> & { ownerSlug: string };

export const USER_SERVICE = new InjectionToken<UserService>('UserService');

export class UserService {
	user: Writable<User | undefined> = writable<User | undefined>(undefined, (set) => {
		this.fetchUser()
			.then((data) => {
				this.error.set(undefined);
				set(data);
			})
			.catch((err) => {
				this.error.set(err);
			});
	});

	readonly error = writable();

	// Patch stack cache for storing user patch stack stores
	private patchStackCache: Map<string, Writable<LoadablePatchStacks>> = new Map();

	constructor(private readonly httpClient: HttpClient) {
		if (!httpClient) {
			console.error('[UserService] HttpClient was not provided in constructor');
		}

		httpClient.authenticationAvailable.subscribe((available) => {
			if (available && get(this.user) === undefined) {
				// If the authentication availability changes, refetch the use
				this.fetchUser()
					.then((data) => {
						this.error.set(undefined);
						this.user.set(data);
					})
					.catch((err) => {
						this.error.set(err);
					});
			}
		});
	}

	private async fetchUser() {
		const user = await this.httpClient.get<User>('/api/user');
		setSentryUser(user);

		return user;
	}

	clearUser() {
		this.user.set(undefined);
	}

	async updateUser(params: {
		name?: string;
		picture?: File;
		website?: string;
		twitter?: string;
		bluesky?: string;
		timezone?: string;
		location?: string;
		emailShare?: boolean;
		readme?: string;
		generate_ssh_token?: boolean;
	}): Promise<any> {
		const formData = new FormData();
		if (params.name) formData.append('name', params.name);
		if (params.picture) formData.append('avatar', params.picture);
		if (params.website !== undefined) formData.append('website', params.website);
		if (params.twitter !== undefined) formData.append('twitter', params.twitter);
		if (params.bluesky !== undefined) formData.append('bluesky', params.bluesky);
		if (params.timezone !== undefined) formData.append('timezone', params.timezone);
		if (params.location !== undefined) formData.append('location', params.location);
		if (params.emailShare !== undefined)
			formData.append('email_share', params.emailShare.toString());
		if (params.readme !== undefined) formData.append('readme', params.readme);
		if (params.generate_ssh_token !== undefined)
			formData.append('generate_ssh_token', params.generate_ssh_token.toString());

		// Content Type must be unset for the right form-data border to be set automatically
		return await this.httpClient.put('user.json', {
			body: formData,
			headers: { 'Content-Type': undefined }
		});
	}

	/**
	 * Gets a store for a user's patch stacks by slug. The store will be populated when accessed.
	 * @param ownerSlug The user slug to fetch patch stacks for
	 * @returns A readable store containing the patch stacks
	 */
	getPatchStacks(ownerSlug: string): Writable<LoadablePatchStacks> {
		if (!this.patchStackCache.has(ownerSlug)) {
			// Create a new store for this user's patch stacks
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
	 * @param ownerSlug The user slug to fetch patch stacks for
	 * @returns Array of patch stacks converted to Branch format
	 */
	async fetchPatchStacks(ownerSlug: string): Promise<Branch[]> {
		// Check if httpClient is defined
		if (!this.httpClient) {
			console.error('[UserService] HttpClient is undefined');
			return [];
		}

		const endpoint = `user/${ownerSlug}/patch_stacks`;

		try {
			const response = await this.httpClient.get<ApiBranch[]>(endpoint);

			// Convert ApiBranch objects to Branch objects
			return response.map(apiToBranch);
		} catch (error: any) {
			console.error(`[UserService] Error with endpoint ${endpoint}:`, error);

			// If this is not a 404 error (which likely means wrong endpoint), rethrow it
			if (error.response && error.response.status !== 404) {
				throw error;
			}

			// If it's a 404, return empty array
			return [];
		}
	}

	/**
	 * Fetches the recent projects for a specific user
	 * @param login The user's login/username
	 * @returns Promise with the user's recent projects
	 */
	async recentProjects(login: string): Promise<any[]> {
		try {
			return await this.httpClient.get<any[]>(`/api/user/${login}/projects?limit=6`);
		} catch (error) {
			console.error('Failed to fetch recent projects:', error);
			return [];
		}
	}
}
