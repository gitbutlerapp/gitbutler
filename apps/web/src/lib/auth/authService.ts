import { persisted } from '$lib/utils/persisted';
import { get } from 'svelte/store';

export class AuthService {
	persistedToken = persisted<string | undefined>(undefined, 'lastProject');

	constructor() {}

	get token() {
		return get(this.persistedToken);
	}

	setToken(data: string) {
		this.persistedToken.set(data);
	}

	clearToken() {
		this.persistedToken.set('');
	}
}
