import { registerInterest } from '$lib/interest/registerInterestFunction.svelte';
import { isFound } from '$lib/network/loadable';
import { UserService } from '$lib/users/userService';
import { usersByLoginSelectors, usersSelectors } from '$lib/users/usersSlice';
import type { Loadable } from '$lib/network/types';
import type { AppUsersState } from '$lib/redux/store.svelte';
import type { Reactive } from '$lib/storeUtils';
import type { User } from '$lib/users/types';

export function getUserByLogin(
	appState: AppUsersState,
	userService: UserService,
	login: string
): Reactive<Loadable<User> | undefined> {
	registerInterest(userService.getUserByLoginInterest(login));
	const currentUserId = $derived(usersByLoginSelectors.selectById(appState.usersByLogin, login));

	const current = $derived.by(() => {
		if (!currentUserId) return undefined;
		if (!isFound(currentUserId)) return currentUserId;
		const id = currentUserId.value;
		return usersSelectors.selectById(appState.users, id);
	});

	return {
		get current() {
			return current;
		}
	};
}
