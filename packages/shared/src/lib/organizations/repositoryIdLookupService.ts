import { InterestStore, type Interest } from '$lib/interest/intrestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { addRepositoryId, upsertRepositoryId } from '$lib/organizations/repositoryIdLookupsSlice';
import { stringifyProjectIdentity } from '$lib/organizations/types';
import { POLLING_GLACIALLY } from '$lib/polling';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

export class RepositoryIdLookupService {
	private readonly projectLookupInterests = new InterestStore<{ identity: string }>(
		POLLING_GLACIALLY
	);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getRepositoryIdInterest(owner: string, slug: string): Interest {
		const identity = stringifyProjectIdentity(owner, slug);
		return this.projectLookupInterests
			.findOrCreateSubscribable({ identity }, async () => {
				this.appDispatch.dispatch(addRepositoryId({ status: 'loading', id: identity }));

				try {
					const { repository_id: repositoryId } = await this.httpClient.get<{
						repository_id: string;
					}>(`projects/lookup/${owner}/${slug}`);

					this.appDispatch.dispatch(
						upsertRepositoryId({
							status: 'found',
							id: identity,
							value: repositoryId
						})
					);
				} catch (error: unknown) {
					this.appDispatch.dispatch(upsertRepositoryId(errorToLoadable(error, identity)));
				}
			})
			.createInterest();
	}
}
