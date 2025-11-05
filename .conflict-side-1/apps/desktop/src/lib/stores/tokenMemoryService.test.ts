import { tokenKey, TokenMemoryService } from '$lib/stores/tokenMemoryService';
import { persisted } from '@gitbutler/shared/persisted';
import { get } from 'svelte/store';
import { test, describe, expect } from 'vitest';
import type { SecretsService } from '$lib/secrets/secretsService';

class MockSecretSevice implements SecretsService {
	constructor(private values: Record<string, string>) {}

	async get(handle: string): Promise<string | undefined> {
		return this.values[handle];
	}

	async set(handle: string, secret: string): Promise<void> {
		this.values[handle] = secret;
	}

	async delete(handle: string): Promise<void> {
		delete this.values[handle];
	}
}

describe('TokenMemoryService', () => {
	test('It gets the value form secret service', async () => {
		const mockSecretSevice = new MockSecretSevice({ [tokenKey]: 'foobar' });
		const tokenMemoryService = new TokenMemoryService(mockSecretSevice);

		const token = await new Promise<string>((resolve) => {
			const sub = tokenMemoryService.token.subscribe((token) => {
				if (token) {
					sub();
					resolve(token);
				}
			});
		});

		expect(token).eq('foobar');
	});

	test('Setting the token to be different', async () => {
		const mockSecretSevice = new MockSecretSevice({ [tokenKey]: 'foobar' });
		const tokenMemoryService = new TokenMemoryService(mockSecretSevice);

		const token = await new Promise<string>((resolve) => {
			const sub = tokenMemoryService.token.subscribe((token) => {
				if (token) {
					sub();
					resolve(token);
				}
			});
		});

		expect(token).eq('foobar');

		await tokenMemoryService.setToken('anotherThing');

		expect(get(tokenMemoryService.token)).eq('anotherThing');
	});

	test('Setting the token to be undefined (logging out)', async () => {
		const mockSecretSevice = new MockSecretSevice({ [tokenKey]: 'foobar' });
		const tokenMemoryService = new TokenMemoryService(mockSecretSevice);

		const token = await new Promise<string>((resolve) => {
			const sub = tokenMemoryService.token.subscribe((token) => {
				if (token) {
					sub();
					resolve(token);
				}
			});
		});

		expect(token).eq('foobar');

		await tokenMemoryService.setToken(undefined);

		expect(get(tokenMemoryService.token)).eq(undefined);
	});

	test('Old tokens in localStorage get ported', async () => {
		const oldToken = persisted<string | undefined>(undefined, tokenKey);
		oldToken.set('foobar');

		const mockSecretSevice = new MockSecretSevice({});
		const tokenMemoryService = new TokenMemoryService(mockSecretSevice);

		expect(get(tokenMemoryService.token)).eq('foobar');
		const oldTokenGotten = await new Promise<undefined>((resolve) => {
			const sub = oldToken.subscribe((token) => {
				if (!token) {
					sub();
					resolve(undefined);
				}
			});
		});
		expect(oldTokenGotten).eq(undefined);
	});
});
