import { get, writable } from 'svelte/store';

export class AuthService {
	token = writable<string | undefined>(undefined);

	constructor() {}

	getToken() {
		return get(this.token);
	}

	setToken(data: string) {
		this.token.set(data);
	}

	clearToken() {
		this.token.set('');
	}
}
