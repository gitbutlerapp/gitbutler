import { HttpClient } from '@gitbutler/shared/network/httpClient';

export interface SshKey {
	id: string;
	name: string;
	fingerprint: string;
	createdAt: string;
}

interface AddSshKeyRequest {
	name: string;
	public_key: string;
}

export class SshKeyService {
	private httpClient: HttpClient;

	constructor(httpClient: HttpClient) {
		this.httpClient = httpClient;
	}

	async getSshKeys(): Promise<SshKey[]> {
		return await this.httpClient.get<SshKey[]>('/api/user/keys');
	}

	async addSshKey(name: string, publicKey: string): Promise<SshKey> {
		const request: AddSshKeyRequest = {
			name,
			public_key: publicKey
		};
		return await this.httpClient.post<SshKey>('/api/user/keys', { body: request });
	}

	async deleteSshKey(fingerprint: string): Promise<void> {
		await this.httpClient.delete(`/api/user/keys?fingerprint=${fingerprint}`);
	}
}
