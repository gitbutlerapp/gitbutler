import { InterestStore, type Interest } from '$lib/interest/intrestStore';
import { POLLING_SLOW } from '$lib/polling';
import { apiToUser, type ApiUser } from '$lib/users/types';
import { upsertUser } from '$lib/users/usersSlice';
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
				const apiUser = await this.httpClient.get<ApiUser>(`user/${login}`);
				const user = apiToUser(apiUser);
				this.appDispatch.dispatch(upsertUser(user));
			})
			.createInterest();
	}
}
