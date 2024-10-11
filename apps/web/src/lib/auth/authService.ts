import { persisted } from '@gitbutler/shared/persisted';
import { type Readable } from 'svelte/store';

export class AuthService {
	#token = persisted<string | undefined>(undefined, 'AuthService--token');

	get token(): Readable<string | undefined> {
		return this.#token;
	}

	setToken(data: string) {
		this.#token.set(data);
	}

	clearToken() {
		this.#token.set(undefined);
	}
}
