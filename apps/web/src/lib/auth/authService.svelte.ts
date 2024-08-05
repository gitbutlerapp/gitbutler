import { PersistedState } from '$lib/persisted.svelte';

export function createAuthService() {
	const persistedToken = new PersistedState('token', '');

	function setToken(data: string) {
		persistedToken.current = data;
	}

	function clearToken() {
		persistedToken.current = '';
	}

	return {
		get token() {
			return persistedToken.current;
		},
		setToken,
		clearToken
	};
}
