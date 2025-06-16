import type { HttpClient } from '$lib/network/httpClient';

interface BaseLoginResponse {
	type: 'success' | 'error';
}

interface SuccessLoginResponse extends BaseLoginResponse {
	type: 'success';
}
interface ErrorLoginResponse extends BaseLoginResponse {
	type: 'error';

	errorCode: string;
	errorMessage: string;
	raw?: unknown;
}

type LoginResponse = SuccessLoginResponse | ErrorLoginResponse;
export default class LoginService {
	constructor(private readonly httpClient: HttpClient) {}

	async loginWithEmail(email: string, password: string): Promise<LoginResponse> {
		try {
			const response = await this.httpClient.postRaw('sessions/login_with_email', {
				body: {
					email,
					password
				}
			});

			if (response.ok) {
				return {
					type: 'success'
				};
			}

			const errorData = await response.json();
			return {
				type: 'error',
				errorCode: errorData.errorCode || 'unknown_error',
				errorMessage: errorData.error || 'An unknown error occurred'
			};
		} catch (error) {
			if (error instanceof Error) {
				return {
					type: 'error',
					errorCode: 'network_error',
					errorMessage: error.message,
					raw: error
				};
			}
			return {
				type: 'error',
				errorCode: 'unknown_error',
				errorMessage: 'An unknown error occurred',
				raw: error
			};
		}
	}
}
