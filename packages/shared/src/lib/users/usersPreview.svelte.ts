import { registerInterest } from '$lib/interest/registerInterestFunction.svelte';
import { UserService } from '$lib/users/userService';
import { usersSelectors } from '$lib/users/usersSlice';
import type { AppUsersState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';
import type { LoadableUser } from '$lib/users/types';

export function getUserByLogin(
	appState: AppUsersState,
	userService: UserService,
	login: string
): Reactive<LoadableUser | undefined> {
	registerInterest(userService.getUserInterest(login));
	const current = $derived(usersSelectors.selectById(appState.users, login));

	return {
		get current() {
			return current;
		}
	};
}
