import { InterestStore, type Interest } from '$lib/interest/interestStore';
import { errorToLoadable } from '$lib/network/loadable';
import { POLLING_SLOW } from '$lib/polling';
import {
	apiToUser,
	apiToUserSimple,
	type ApiUser,
	type LoadableUser,
	type LoadableUserIdByLogin,
	type SearchUsersApiParams,
	type UserSimple
} from '$lib/users/types';
import { userTable, userByLoginTable } from '$lib/users/usersSlice';
import { InjectionToken } from '../context';
import type { HttpClient } from '$lib/network/httpClient';
import type { AppDispatch } from '$lib/redux/store.svelte';

export const USER_SERVICE_TOKEN = new InjectionToken<UserService>('UserService');

export class UserService {
	private readonly userInterests = new InterestStore<{ id: number }>(POLLING_SLOW);
	private readonly userByLoginInterests = new InterestStore<{ login: string }>(POLLING_SLOW);

	constructor(
		private readonly httpClient: HttpClient,
		private readonly appDispatch: AppDispatch
	) {}

	getUserInterest(id: number): Interest {
		return this.userInterests
			.findOrCreateSubscribable({ id }, async () => {
				this.appDispatch.dispatch(userTable.addOne({ status: 'loading', id }));

				try {
					const apiUsers = await this.httpClient.post<ApiUser[]>(`user_search`, {
						body: { id }
					});

					if (apiUsers.length === 0) {
						this.appDispatch.dispatch(userTable.upsertOne({ status: 'not-found', id }));
						return;
					}

					if (apiUsers.length !== 1) {
						// should not happen
						throw new Error(`Expected 1 user, got ${apiUsers.length}`);
					}

					const apiUser = apiUsers[0]!;
					const user = apiToUser(apiUser);
					this.appDispatch.dispatch(userTable.upsertOne({ status: 'found', id, value: user }));
					if (user.login) {
						this.appDispatch.dispatch(
							userByLoginTable.upsertOne({ status: 'found', id: user.login, value: user.id })
						);
					}
				} catch (error: unknown) {
					this.appDispatch.dispatch(userTable.addOne(errorToLoadable(error, id)));
				}
			})
			.createInterest();
	}

	getUserByLoginInterest(login: string): Interest {
		return this.userByLoginInterests
			.findOrCreateSubscribable({ login }, async () => {
				try {
					const apiUsers = await this.httpClient.post<ApiUser[]>(`user_search`, {
						body: { login }
					});

					if (apiUsers.length === 0) {
						this.appDispatch.dispatch(
							userByLoginTable.upsertOne({ status: 'not-found', id: login })
						);
						return;
					}

					if (apiUsers.length !== 1) {
						// should not happen
						throw new Error(`Expected 1 user, got ${apiUsers.length}`);
					}

					const apiUser = apiUsers[0]!;
					const user = apiToUser(apiUser);
					this.appDispatch.dispatch(
						userByLoginTable.upsertOne({ status: 'found', id: login, value: user.id })
					);
					this.appDispatch.dispatch(
						userTable.upsertOne({ status: 'found', id: user.id, value: user })
					);
				} catch (error: unknown) {
					this.appDispatch.dispatch(userByLoginTable.addOne(errorToLoadable(error, login)));
				}
			})
			.createInterest();
	}

	async searchUsers(params: SearchUsersApiParams): Promise<UserSimple[]> {
		const apiUsers = await this.httpClient.post<ApiUser[]>(`user_search`, {
			body: params
		});

		// Store the found users in the redux store
		const users = apiUsers.map(apiToUser);

		const loadableUsers = users.map(
			(user): LoadableUser => ({ status: 'found', id: user.id, value: user })
		);
		this.appDispatch.dispatch(userTable.upsertMany(loadableUsers));

		const loadableUsersByLogin = users
			.map((user): LoadableUserIdByLogin | undefined => {
				if (!user.login) return undefined;
				return { status: 'found', id: user.login, value: user.id };
			})
			.filter((loadable): loadable is LoadableUserIdByLogin => !!loadable);

		this.appDispatch.dispatch(userByLoginTable.upsertMany(loadableUsersByLogin));

		return apiUsers.map(apiToUserSimple);
	}
}
