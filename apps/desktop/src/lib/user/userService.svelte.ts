import { resetSentry, setSentryUser } from "$lib/analytics/sentry";
import { showError } from "$lib/error/showError";
import { type UiState } from "$lib/state/uiState.svelte";
import { InjectionToken } from "@gitbutler/core/context";
import { chipToasts } from "@gitbutler/ui";
import type { IBackend } from "$lib/backend";
import type { BackendApi } from "$lib/state/backendApi";
import type { PostHogWrapper } from "$lib/telemetry/posthog";
import type { TokenMemoryService } from "$lib/user/tokenMemoryService";
import type { User } from "$lib/user/user";
import type { ApiUser } from "@gitbutler/shared/users/types";

export const USER_SERVICE = new InjectionToken<UserService>("UserService");

export class UserService {
	private _incomingUserLogin = $state<User | undefined>(undefined);
	private userQuery;

	get user(): User | undefined {
		return this.userQuery.response;
	}

	get incomingUserLogin(): User | undefined {
		return this._incomingUserLogin;
	}

	constructor(
		private backendApi: BackendApi,
		private backend: IBackend,
		private tokenMemoryService: TokenMemoryService,
		private posthog: PostHogWrapper,
		private uiState: UiState,
	) {
		this.userQuery = this.backendApi.endpoints.getUser.useQuery(undefined);

		$effect(() => {
			if (!this.userQuery.result.isSuccess) return;

			const user = this.userQuery.response;
			if (user) {
				this.tokenMemoryService.setToken(user.access_token);
				this.setUserTelemetry(user);
			} else {
				this.posthog.setAnonymousPostHogUser();
			}
		});
	}

	async setUser(user: User | undefined) {
		if (user) {
			await this.backendApi.endpoints.setUser.mutate({ user });
			this.tokenMemoryService.setToken(user.access_token);
			await this.setUserTelemetry(user);
		} else {
			await this.backendApi.endpoints.deleteUser.mutate(undefined);
		}
	}

	private async setUserTelemetry(user: User) {
		await this.posthog.setPostHogUser({ id: user.id, email: user.email, name: user.name });
		setSentryUser(user);
	}

	async setUserAccessToken(token: string, bypassConfirmationToast = false) {
		try {
			const currentUser = await this.backendApi.endpoints.getUser.fetch(undefined, {
				forceRefetch: true,
			});
			if (currentUser) {
				showError(
					"Error: Attempting to log in before logging out first",
					"There's already an account logged in, please log out before attempting to log in to another account.",
				);
				return;
			}

			const user = await this.backendApi.endpoints.loginWithToken.mutate({ token });

			if (bypassConfirmationToast) {
				await this.setUser(user);
				return;
			}

			this._incomingUserLogin = user;
			// Display a login confirmation modal
			this.uiState.global.modal.set({
				type: "login-confirmation",
			});
		} catch (error) {
			console.error("Error setting user access token", error);
			showError("Error occurred while logging in", error);
		}
	}

	async acceptIncomingUser(incomingUser: User) {
		if (!incomingUser) {
			throw new Error("No incoming user to accept");
		}
		await this.setUser(incomingUser);
		this._incomingUserLogin = undefined;
	}

	async rejectIncomingUser() {
		this._incomingUserLogin = undefined;
	}

	async forgetUserCredentials() {
		await this.backendApi.endpoints.deleteUser.mutate(undefined);
		this.tokenMemoryService.setToken(undefined);
		await this.posthog.resetPostHog();
		resetSentry();
	}

	private async getLoginUrl(): Promise<string> {
		await this.forgetUserCredentials();
		try {
			const loginToken = await this.backendApi.endpoints.getLoginToken.fetch(undefined, {
				forceRefetch: true,
			});
			const url = new URL(loginToken.url);
			const buildType = await this.backend.invoke<string>("build_type").catch(() => undefined);
			if (buildType !== undefined && buildType !== "development")
				url.searchParams.set("bt", buildType);

			return url.toString();
		} catch (err) {
			console.error(err);
			showError("Error occurred while fetching the login URL", err);
			throw err;
		}
	}

	async openLoginPage(): Promise<void> {
		const url = await this.getLoginUrl();
		await this.backend.openExternalUrl(url);
	}

	async copyLoginPageLink(): Promise<void> {
		const url = await this.getLoginUrl();
		await this.backend
			.writeTextToClipboard(url)
			.then(() => {
				chipToasts.success("Login URL copied to clipboard");
			})
			.catch((err) => {
				showError("Error copying login URL to clipboard", err);
				throw err;
			});
	}

	async getUser(): Promise<ApiUser> {
		return await this.backendApi.endpoints.getUserProfile.fetch(undefined);
	}

	async updateUser(params: {
		name?: string;
		picture?: File;
		website?: string;
		twitter?: string;
		bluesky?: string;
		timezone?: string;
		location?: string;
		emailShare?: boolean;
	}): Promise<any> {
		let avatarBase64: string | undefined;
		let avatarFilename: string | undefined;
		if (params.picture) {
			const bytes = new Uint8Array(await params.picture.arrayBuffer());
			const chunks: string[] = [];
			for (let i = 0; i < bytes.length; i += 0x8000) {
				chunks.push(String.fromCharCode(...bytes.subarray(i, i + 0x8000)));
			}
			avatarBase64 = btoa(chunks.join(""));
			avatarFilename = params.picture.name;
		}

		return await this.backendApi.endpoints.updateUserProfile.mutate({
			params: {
				name: params.name,
				website: params.website,
				twitter: params.twitter,
				bluesky: params.bluesky,
				timezone: params.timezone,
				location: params.location,
				email_share: params.emailShare,
				avatar_base64: avatarBase64,
				avatar_filename: avatarFilename,
			},
		});
	}
}
