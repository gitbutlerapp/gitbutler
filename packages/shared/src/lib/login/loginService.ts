import { isStr } from '@gitbutler/ui/utils/string';
import type { HttpClient } from '$lib/network/httpClient';

interface BaseLoginResponse {
	type: 'success' | 'error';
}

interface SuccessLoginResponse<T> extends BaseLoginResponse {
	type: 'success';
	data: T;
}
interface ErrorLoginResponse extends BaseLoginResponse {
	type: 'error';

	errorCode: string;
	errorMessage: string;
	raw?: unknown;
}

type LoginResponse<T = void> = SuccessLoginResponse<T> | ErrorLoginResponse;
export default class LoginService {
	constructor(private readonly httpClient: HttpClient) {}

	async loginWithEmail(email: string, password: string): Promise<LoginResponse<string>> {
		try {
			const response = await this.httpClient.postRaw('sessions/login_with_email', {
				body: {
					email,
					password
				}
			});

			if (response.ok) {
				const data = await response.json();

				if (!isStr(data.token)) throw new Error('Invalid token format');

				return {
					type: 'success',
					data: data.token
				};
			}

			const errorData = await response.json();
			return {
				type: 'error',
				errorCode: errorData.error_code || 'unknown_error',
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

	async resendConfirmationEmail(email: string): Promise<LoginResponse<{ message: string }>> {
		try {
			const response = await this.httpClient.postRaw('sessions/resend_confirmation', {
				body: { email }
			});

			if (response.ok) {
				const data = await response.json();
				if (!isStr(data.message)) {
					throw new Error('Invalid message format');
				}
				return {
					type: 'success',
					data: { message: data.message }
				};
			}

			const errorData = await response.json();
			return {
				type: 'error',
				errorCode: errorData.error_code || 'unknown_error',
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

	async createAccountWithEmail(
		email: string,
		password: string,
		passwordConfirmation: string
	): Promise<LoginResponse> {
		try {
			const response = await this.httpClient.postRaw('sessions/sign_up_email', {
				body: {
					email,
					password,
					password_confirmation: passwordConfirmation
				}
			});

			if (response.ok) {
				return {
					type: 'success',
					data: undefined
				};
			}

			const errorData = await response.json();
			return {
				type: 'error',
				errorCode: errorData.error_code || 'unknown_error',
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
