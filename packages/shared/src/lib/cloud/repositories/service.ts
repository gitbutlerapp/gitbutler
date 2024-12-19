import { writableDerived } from '$lib/storeUtils';
import { derived, get, type Readable, type Writable } from 'svelte/store';
import type { HttpClient } from '$lib/network/httpClient';

interface ApiRepository {
	name: string;
	description: string | null;
	repository_id: string;
	git_url: string;
	created_at: string;
	updated_at: string;
}

export class CloudRepository {
	readonly name: string;
	readonly description: string | null;
	readonly repositoryId: string;
	readonly gitUrl: string;
	readonly createdAt: Date;
	readonly updatedAt: Date;

	constructor(apiRepository: ApiRepository) {
		this.name = apiRepository.name;
		this.description = apiRepository.description;
		this.repositoryId = apiRepository.repository_id;
		this.gitUrl = apiRepository.git_url;
		this.createdAt = new Date(apiRepository.created_at);
		this.updatedAt = new Date(apiRepository.updated_at);
	}
}

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
