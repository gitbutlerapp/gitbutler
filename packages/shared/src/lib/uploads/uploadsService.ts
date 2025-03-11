import { apiToUpload, type ApiUpload, type Upload } from './types';
import type { HttpClient } from '$lib/network/httpClient';

export class UploadsService {
	constructor(private readonly httpClient: HttpClient) {}

	async uploadFile(file: File): Promise<Upload> {
		const formData = new FormData();
		formData.append('file', file);
		formData.append('public', 'true');

		const response = await this.httpClient.post<ApiUpload>('uploads', {
			body: formData,
			headers: { 'Content-Type': undefined }
		});

		return apiToUpload(response);
	}
}
