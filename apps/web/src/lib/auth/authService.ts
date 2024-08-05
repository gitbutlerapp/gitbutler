import { get, writable } from 'svelte/store';

export class AuthService {
	token = writable<string | undefined>(undefined);

	constructor() {}

	getToken() {
		return get(this.token);
	}

	setToken(data: string) {
		console.log('data', data);
		this.token.set(data);
		console.log('data2', get(this.token));
	}

	clearToken() {
		this.token.set('');
	}
}
