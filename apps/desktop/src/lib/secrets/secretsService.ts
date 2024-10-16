import { AISecretHandle } from '$lib/ai/service';
import { invoke } from '$lib/backend/ipc';
import { buildContext } from '@gitbutler/shared/context';
import type { GitConfigService } from '$lib/backend/gitConfigService';

const MIGRATION_HANDLES = [
	AISecretHandle.AnthropicKey.toString(),
	AISecretHandle.OpenAIKey.toString()
];

export type SecretsService = {
	get(handle: string): Promise<string | undefined>;
	set(handle: string, secret: string): Promise<void>;
};

export const [getSecretsService, setSecretsService] =
	buildContext<SecretsService>('secretsService');

export class RustSecretService implements SecretsService {
	constructor(private gitConfigService: GitConfigService) {}

	async get(handle: string) {
		const secret = await invoke<string>('secret_get_global', { handle });
		if (secret) return secret;

		if (MIGRATION_HANDLES.includes(handle)) {
			const key = 'gitbutler.' + handle;
			const migratedSecret = await this.migrate(key, handle);
			if (migratedSecret !== undefined) return migratedSecret;
		}
	}

	async set(handle: string, secret: string) {
		await invoke('secret_set_global', {
			handle,
			secret
		});
	}

	/**
	 * Migrates a specific key from git config to secrets.
	 *
	 * TODO: Remove this code and the dependency on GitConfigService in the future.
	 */
	private async migrate(key: string, handle: string): Promise<string | undefined> {
		const secretInConfig = await this.gitConfigService.get(key);
		if (secretInConfig === undefined) return;

		await this.set(handle, secretInConfig);
		await this.gitConfigService.remove(key);

		console.warn(`Migrated Git config "${key}" to secret store.`);
		return secretInConfig;
	}
}
