import { parseQueryError } from "$lib/error/error";
import { InjectionToken } from "@gitbutler/core/context";
import { posthog, type PostHog, type Properties } from "posthog-js";
import type { IBackend } from "$lib/backend";
import type { RepoInfo } from "$lib/git/gitUrl";
import type { SettingsService } from "$lib/settings/appSettings";
import type { EventContext } from "$lib/telemetry/eventContext";
import { PUBLIC_POSTHOG_API_KEY } from "$env/static/public";

export const POSTHOG_WRAPPER = new InjectionToken<PostHogWrapper>("PostHogWrapper");

export class PostHogWrapper {
	private _instance: PostHog | void = undefined;

	constructor(
		private settingsService: SettingsService,
		private backend: IBackend,
		private eventContext: EventContext,
	) {}

	capture(eventName: string, properties?: Properties) {
		const sampled = sampleEvent(eventName, properties);
		if (!sampled) return;
		const context = this.eventContext.getAll();
		const newProperties = { ...context, ...sampled };
		const skipClientRateLimiting = eventName === "tauri_command" && sampled.command !== undefined;
		this._instance?.capture(eventName, newProperties, {
			skip_client_rate_limiting: skipClientRateLimiting,
		});
	}

	captureOnboarding(event: OnboardingEvent, error?: unknown, extraProperties?: Properties) {
		const context = this.eventContext.getAll();
		const parsedError = error === undefined ? undefined : parseQueryError(error);
		const properties = {
			...context,
			...extraProperties,
			...(parsedError && {
				error_title: parsedError.name,
				error_message: parsedError.message,
				error_code: parsedError.code,
			}),
		};
		this._instance?.capture(event, properties);
	}

	captureAction(event: ActionEvent, properties?: Properties) {
		const context = this.eventContext.getAll();
		const newProperties = { ...context, ...properties };
		this._instance?.capture(event, newProperties);
	}

	async init() {
		if (this._instance) return;
		const appInfo = await this.backend.getAppInfo();
		this._instance = posthog.init(PUBLIC_POSTHOG_API_KEY, {
			api_host: "https://eu.posthog.com",
			autocapture: false,
			disable_session_recording: true,
			capture_performance: false,
			request_batching: true,
			persistence: "localStorage",
			on_xhr_error: (e) => {
				console.error("posthog error", e);
			},
		});
		posthog.register({
			appName: appInfo.name,
			appVersion: appInfo.version,
		});
	}

	async setPostHogUser(params: { id: number; email?: string; name?: string }) {
		const { id, email, name } = params;
		const distinctId = `user_${id}`;
		this._instance?.identify(distinctId, {
			email,
			name,
		});
		await this.settingsService.updateTelemetryDistinctId(distinctId);
	}

	setAnonymousPostHogUser() {
		if (this._instance) {
			const distinctId = this._instance.get_distinct_id();
			this.settingsService.updateTelemetryDistinctId(distinctId);
		}
	}

	async resetPostHog() {
		this._instance?.capture("logout");
		this._instance?.reset();
		await this.settingsService.updateTelemetryDistinctId(null);
	}

	/**
	 * Include repo information for all events for the remainder of the session,
	 * or until cleared.
	 */
	setPostHogRepo(repo: RepoInfo | undefined) {
		if (repo) {
			this._instance?.register_for_session({ repoDomain: repo.domain, repoHash: repo.hash });
		} else {
			this._instance?.unregister_for_session("repoDomain");
			this._instance?.unregister_for_session("repoHash");
		}
	}
}

/**
 * Sampling rates for high-volume `tauri_command` events, in (0, 1].
 *
 * Failures are sampled like successes so a persistent failure storm on one of
 * these commands cannot flood PostHog at full rate. Sampled events are stamped
 * with their `samplingRate`; estimate totals in PostHog with
 * `sum(1 / coalesce(samplingRate, 1))`.
 */
const SAMPLED_COMMANDS = new Map<string, number>([
	["head_info", 0.05],
	["get_base_branch_data", 0.5],
	["workspace_fetch_from_remotes", 0.5],
]);

/**
 * Applies per-command sampling: returns the properties to capture (stamped
 * with the effective `samplingRate` for sampled commands), or null when the
 * event should be dropped.
 */
export function sampleEvent(
	eventName: string,
	properties: Properties | undefined,
	draw = Math.random(),
): Properties | null {
	const rate =
		eventName === "tauri_command" && typeof properties?.command === "string"
			? SAMPLED_COMMANDS.get(properties.command)
			: undefined;
	if (rate === undefined) return properties ?? {};
	if (draw >= rate) return null;
	return { ...properties, samplingRate: rate };
}

export enum OnboardingEvent {
	ConfirmedAnalytics = "onboarding_confirmed_analytics",
	AddLocalProject = "onboarding_add_local_project",
	AddLocalProjectFailed = "onboarding_add_local_project_failed",
	ClonedProject = "onboarding_cloned_project",
	ClonedProjectFailed = "onboarding_cloned_project_failed",
	ProjectSetupContinue = "onboarding_project_setup_continue",
	SetTargetBranch = "onboarding_set_target_branch",
	SetTargetBranchFailed = "onboarding_set_target_branch_failed",
	SetProjectActive = "onboarding_set_project_active",
	SetProjectActiveFailed = "onboarding_set_project_active_failed",
	LoginGitButler = "onboarding_login_gitbutler",
	CancelLoginGitButler = "onboarding_cancel_login_gitbutler",
	GitHubInitiateOAuth = "onboarding_github_initiate_oauth",
	GitHubStorePat = "onboarding_github_store_pat",
	GitLabStorePat = "onboarding_gitlab_store_pat",
	GitHubStoreGHEPat = "onboarding_github_store_ghe_pat",
	GitLabStoreSelfHostedPat = "onboarding_gitlab_store_self_hosted_pat",
	GitHubOAuthFailed = "onboarding_github_oauth_failed",
	GitHubStorePatFailed = "onboarding_github_store_pat_failed",
	GitLabStorePatFailed = "onboarding_gitlab_store_pat_failed",
	GitHubStoreGHEPatFailed = "onboarding_github_store_ghe_pat_failed",
	GitLabStoreSelfHostedPatFailed = "onboarding_gitlab_store_self_hosted_pat_failed",
	BitbucketStoreApiToken = "onboarding_bitbucket_store_api_token",
	BitbucketStoreApiTokenFailed = "onboarding_bitbucket_store_api_token_failed",
	GitCheckCredentials = "onboarding_git_check_credentials",
	GitCheckCredentialsFailed = "onboarding_git_check_credentials_failed",
	GitAuthenticationContinue = "onboarding_git_authentication_continue",
}

export enum ActionEvent {
	CommitToNewBranch = "action_commit_to_new_branch",
}
