import { resetSentry, setSentryUser } from "$lib/analytics/sentry";
import { showError } from "$lib/notifications/toasts";
import { type UiState } from "$lib/state/uiState.svelte";
import { InjectionToken } from "@gitbutler/core/context";
import { type HttpClient } from "@gitbutler/shared/network/httpClient";
import { chipToasts } from "@gitbutler/ui";
import { derived, writable, type Readable } from "svelte/store";
import type { PostHogWrapper } from "$lib/analytics/posthog";
import type { IBackend } from "$lib/backend";
import type { TokenMemoryService } from "$lib/stores/tokenMemoryService";
import type { User } from "$lib/user/user";
import type { ApiUser } from "@gitbutler/shared/users/types";

export type LoginToken = {
	/** Used for polling the user; should NEVER be sent to the browser. */
	token: string;
	browser_token: string;
	expires: string;
	url: string;
};

export const USER_SERVICE = new InjectionToken<UserService>("UserService");

export class UserService {
	readonly loading = writable(false);

	readonly user = writable<User | undefined>(undefined, () => {
		this.refresh();
	});
	readonly userLogin = derived<Readable<User | undefined>, string | undefined>(
		this.user,
		(user, set) => {
			if (user) {
				this.getUser().then((user) => set(user.login ?? undefined));
			} else {
				set(undefined);
			}
		},
	);
	readonly error = writable();
	readonly incomingUserLogin = writable<User | undefined>(undefined);

	async refresh() {
		const user = await this.backend.invoke<User | undefined>("get_user");
		if (user) {
			this.tokenMemoryService.setToken(user.access_token);
			// Telemetry is alreary set when the user is set.
			// Just in case the user ID changes in the backend outside the usual cycle, we set it again here.
			await this.setUserTelemetry(user);
			this.user.set(user);
			return user;
		}

		this.posthog.setAnonymousPostHogUser();
		this.user.set(undefined);
	}

	constructor(
		private backend: IBackend,
		private httpClient: HttpClient,
		private tokenMemoryService: TokenMemoryService,
		private posthog: PostHogWrapper,
		private uiState: UiState,
	) {}

	async setUser(user: User | undefined) {
		if (user) {
			await this.backend.invoke("set_user", { user });
			this.tokenMemoryService.setToken(user.access_token);
			await this.setUserTelemetry(user);
		} else {
			await this.clearUser();
		}
		this.user.set(user);
	}

	private async clearUser() {
		await this.backend.invoke("delete_user");
	}

	private async setUserTelemetry(user: User) {
		await this.posthog.setPostHogUser({ id: user.id, email: user.email, name: user.name });
		setSentryUser(user);
	}

	async setUserAccessToken(token: string, bypassConfirmationToast = false) {
		try {
			const user = await this.httpClient.get<User>("login/whoami", {
				headers: {
					"X-Auth-Token": token,
				},
			});

			if (bypassConfirmationToast) {
				// In the case that the token is e.g. pasted directly, we don't need a confirmation toast.
				await this.setUser(user);
				return;
			}

			this.incomingUserLogin.set(user);
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
		this.incomingUserLogin.set(undefined);
	}

	async rejectIncomingUser() {
		this.incomingUserLogin.set(undefined);
	}

	async forgetUserCredentials() {
		await this.clearUser();
		this.user.set(undefined);
		this.tokenMemoryService.setToken(undefined);
		await this.posthog.resetPostHog();
		resetSentry();
	}

	private async getLoginUrl(): Promise<string> {
		this.forgetUserCredentials();
		try {
			// Get the login url from the backend
			const token = await this.httpClient.post<LoginToken>("login/token.json");
			const url = new URL(token.url);
			url.host = this.httpClient.apiUrl.host;
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
		return await this.httpClient.get("user.json");
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
		const formData = new FormData();
		if (params.name) formData.append("name", params.name);
		if (params.picture) formData.append("avatar", params.picture);
		if (params.website !== undefined) formData.append("website", params.website);
		if (params.twitter !== undefined) formData.append("twitter", params.twitter);
		if (params.bluesky !== undefined) formData.append("bluesky", params.bluesky);
		if (params.timezone !== undefined) formData.append("timezone", params.timezone);
		if (params.location !== undefined) formData.append("location", params.location);
		if (params.emailShare !== undefined)
			formData.append("email_share", params.emailShare.toString());

		// Content Type must be unset for the right form-data border to be set automatically
		return await this.httpClient.put("user.json", {
			body: formData,
			headers: { "Content-Type": undefined },
		});
	}
}
