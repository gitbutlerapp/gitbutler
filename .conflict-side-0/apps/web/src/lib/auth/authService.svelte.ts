import { persisted } from '@gitbutler/shared/persisted';
import { readableToReactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { type Readable } from 'svelte/store';
import type { Reactive } from '@gitbutler/shared/storeUtils';

export class AuthService {
	#token = persisted<string | undefined>(undefined, 'AuthService--token');

	get tokenReadable(): Readable<string | undefined> {
		return this.#token;
	}

	get token(): Reactive<string | undefined> {
		return readableToReactive(this.tokenReadable);
	}

	setToken(data: string) {
		this.#token.set(data);
	}

	clearToken() {
		this.#token.set(undefined);
	}
}
