import { registerInterest } from '$lib/interest/registerInterestFunction.svelte';
import { isFound } from '$lib/network/loadable';
import { UserService } from '$lib/users/userService';
import { userTable, userByLoginTable } from '$lib/users/usersSlice';
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
	const currentUserId = $derived(
		userByLoginTable.selectors.selectById(appState.usersByLogin, login)
	);

	const current = $derived.by(() => {
		if (!currentUserId) return undefined;
		if (!isFound(currentUserId)) return currentUserId;
		const id = currentUserId.value;
		return userTable.selectors.selectById(appState.users, id);
	});

	return {
		get current() {
			return current;
		}
	};
}
