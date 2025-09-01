import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { repositoryIdLookupTable } from '$lib/organizations/repositoryIdLookupsSlice';
import { stringifyProjectIdentity } from '$lib/organizations/types';
import { POLLING_GLACIALLY } from '$lib/polling';
import { InjectionToken } from '@gitbutler/core/context';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

export const REPOSITORY_ID_LOOKUP_SERVICE: InjectionToken<RepositoryIdLookupService> =
	new InjectionToken('RepositoryIdLookupService');

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
				this.appDispatch.dispatch(
					repositoryIdLookupTable.addOne({ status: 'loading', id: identity })
				);

				try {
					const { repository_id: repositoryId } = await this.httpClient.get<{
						repository_id: string;
					}>(`projects/lookup/${owner}/${slug}`);

					this.appDispatch.dispatch(
						repositoryIdLookupTable.upsertOne({
							status: 'found',
							id: identity,
							value: repositoryId
						})
					);
				} catch (error: unknown) {
					this.appDispatch.dispatch(
						repositoryIdLookupTable.addOne(errorToLoadable(error, identity))
					);
				}
			})
			.createInterest();
	}
}
