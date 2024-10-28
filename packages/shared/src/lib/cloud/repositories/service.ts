import { type ApiRepository, CloudRepository } from '$lib/cloud/types';
import { writableDerived } from '$lib/storeUtils';
import { derived, get, type Readable, type Writable } from 'svelte/store';
import type { HttpClient } from '$lib/httpClient';

export class RepositoriesApiService {
	readonly canGetRepositories: Readable<boolean>;

	constructor(private readonly httpClient: HttpClient) {
		this.canGetRepositories = httpClient.authenticationAvailable;
	}

	async getRepositories(): Promise<ApiRepository[] | undefined> {
		try {
			return await this.httpClient.get<ApiRepository[]>('projects');
		} catch (e) {
			if (e instanceof TypeError) {
				return undefined;
			} else {
				throw e;
			}
		}
	}
}

const MINUTES_15 = 15 * 60 * 1000;

export class CloudRepositoriesService {
	readonly #apiRepositories: Writable<ApiRepository[] | undefined>;

	readonly repositories: Readable<CloudRepository[] | undefined>;

	constructor(private readonly repositoriesApiService: RepositoriesApiService) {
		this.#apiRepositories = writableDerived(
			repositoriesApiService.canGetRepositories,
			undefined as ApiRepository[] | undefined,
			(canGetRepositories, set) => {
				if (!canGetRepositories) {
					set(undefined);
					return;
				}

				let canceled = false;

				const callback = (() => {
					this.repositoriesApiService.getRepositories().then((apiRepositories) => {
						if (!canceled) {
							set(apiRepositories);
						}
					});
				}).bind(this);

				callback();
				const timeout = setInterval(callback, MINUTES_15);

				return () => {
					canceled = true;
					clearInterval(timeout);
				};
			}
		);

		this.repositories = derived(this.#apiRepositories, (apiRepositories) => {
			return apiRepositories?.map((apiRepository) => new CloudRepository(apiRepository));
		});
	}

	/** Refresh the list of patch stacks */
	async refresh(): Promise<void> {
		const canGetRepositories = get(this.repositoriesApiService.canGetRepositories);

		if (canGetRepositories) {
			const repositories = await this.repositoriesApiService.getRepositories();
			this.#apiRepositories.set(repositories);
		} else {
			this.#apiRepositories.set(undefined);
		}
	}
}
