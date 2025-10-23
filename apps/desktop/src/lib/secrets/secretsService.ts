import { InjectionToken } from '@gitbutler/core/context';
import type { IBackend } from '$lib/backend';

export type SecretsService = {
	get(handle: string): Promise<string | undefined>;
	set(handle: string, secret: string): Promise<void>;
	delete(handle: string): Promise<void>;
};

export const SECRET_SERVICE = new InjectionToken<SecretsService>('SecretService');

export class RustSecretService implements SecretsService {
	constructor(private backend: IBackend) {}

	async get(handle: string) {
		const secret = await this.backend.invoke<string>('secret_get', { handle });
		if (secret) return secret;
	}

	async set(handle: string, secret: string) {
		await this.backend.invoke('secret_set', {
			handle,
			secret
		});
	}

	async delete(handle: string) {
		await this.backend.invoke('secret_delete', { handle });
	}
}
