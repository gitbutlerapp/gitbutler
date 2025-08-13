import { InjectionToken } from '$lib/context';
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

export const LOGIN_SERVICE = new InjectionToken<LoginService>('LoginService');
export default class LoginService {
	constructor(private readonly httpClient: HttpClient) {}

	private async sendPostRequest<T>(
		path: string,
		body: Record<string, unknown>,
		successHandler?: (data: any) => T | undefined
	): Promise<LoginResponse<T>> {
		try {
			const response = await this.httpClient.postRaw(path, { body });

			const data = await response.json();

			if (response.ok) {
				const result = successHandler ? successHandler(data) : data;
				return {
					type: 'success',
					data: result as T
				};
			}

			return {
				type: 'error',
				errorCode: data.error_code || 'unknown_error',
				errorMessage: data.error || 'An unknown error occurred'
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

	async finalizeAccount(
		token: string,
		email: string,
		username: string
	): Promise<LoginResponse<{ message: string }>> {
		return await this.sendPostRequest(
			'sessions/finalize',
			{
				token,
				email,
				login: username
			},
			(data) => {
				if (!isStr(data.message)) throw new Error('Invalid message format');
				return { message: data.message };
			}
		);
	}

	async confirmPasswordReset(
		token: string,
		newPassword: string,
		passwordConfirmation: string
	): Promise<LoginResponse<{ message: string; token: string }>> {
		return await this.sendPostRequest(
			'sessions/confirm_new_password',
			{
				password_reset_token: token,
				password: newPassword,
				password_confirmation: passwordConfirmation
			},
			(data) => {
				if (!isStr(data.message)) throw new Error('Invalid message format');
				if (!isStr(data.token)) throw new Error('Invalid token format');
				return { message: data.message, token: data.token };
			}
		);
	}

	async resetPassword(email: string): Promise<LoginResponse<{ message: string }>> {
		return await this.sendPostRequest('sessions/forgot_password', { email }, (data) => {
			if (!isStr(data.message)) throw new Error('Invalid message format');
			return { message: data.message };
		});
	}

	async loginWithEmail(email: string, password: string): Promise<LoginResponse<string>> {
		return await this.sendPostRequest('sessions/login_with_email', { email, password }, (data) => {
			if (!isStr(data.token)) throw new Error('Invalid token format');
			return data.token;
		});
	}

	async resendConfirmationEmail(email: string): Promise<LoginResponse<{ message: string }>> {
		return await this.sendPostRequest('sessions/resend_confirmation', { email }, (data) => {
			if (!isStr(data.message)) throw new Error('Invalid message format');
			return { message: data.message };
		});
	}

	async createAccountWithEmail(
		email: string,
		password: string,
		passwordConfirmation: string
	): Promise<LoginResponse<{ message: string }>> {
		return await this.sendPostRequest(
			'sessions/sign_up_email',
			{
				email,
				password,
				password_confirmation: passwordConfirmation
			},
			(data) => {
				if (!isStr(data.message)) throw new Error('Invalid message format');
				return { message: data.message };
			}
		);
	}
}
