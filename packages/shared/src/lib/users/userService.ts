import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { POLLING_SLOW } from '$lib/polling';
import { apiToUser, type ApiUser } from '$lib/users/types';
import { addUser, upsertUser } from '$lib/users/usersSlice';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

export class UserService {
	private readonly userInterests = new InterestStore<{ login: string }>(POLLING_SLOW);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getUserInterest(login: string): Interest {
		return this.userInterests
			.findOrCreateSubscribable({ login }, async () => {
				this.appDispatch.dispatch(addUser({ status: 'loading', id: login }));

				try {
					const apiUser = await this.httpClient.get<ApiUser>(`user/${login}`);
					const user = apiToUser(apiUser);
					this.appDispatch.dispatch(upsertUser({ status: 'found', id: login, value: user }));
				} catch (error: unknown) {
					this.appDispatch.dispatch(upsertUser(errorToLoadable(error, login)));
				}
			})
			.createInterest();
	}
}
