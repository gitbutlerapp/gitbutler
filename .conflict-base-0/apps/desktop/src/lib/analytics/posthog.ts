import { parseQueryError } from '$lib/error/error';
import { InjectionToken } from '@gitbutler/core/context';
import { PostHog, posthog, type Properties } from 'posthog-js';
import type { EventContext } from '$lib/analytics/eventContext';
import type { IBackend } from '$lib/backend';
import type { SettingsService } from '$lib/config/appSettingsV2';
import type { RepoInfo } from '$lib/url/gitUrl';
import { PUBLIC_POSTHOG_API_KEY } from '$env/static/public';

export const POSTHOG_WRAPPER = new InjectionToken<PostHogWrapper>('PostHogWrapper');

export class PostHogWrapper {
	private _instance: PostHog | void = undefined;

	constructor(
		private settingsService: SettingsService,
		private backend: IBackend,
		private eventContext: EventContext
	) {}

	capture(eventName: string, properties?: Properties) {
		const context = this.eventContext.getAll();
		const newProperties = { ...context, ...properties };
		this._instance?.capture(eventName, newProperties);
	}

	captureOnboarding(event: OnboardingEvent, error?: unknown) {
		const context = this.eventContext.getAll();
		const parsedError = parseQueryError(error);
		const properties = {
			...context,
			error_title: parsedError.name,
			error_message: parsedError.message,
			error_code: parsedError.code
		};
		this._instance?.capture(event, properties);
	}

	async init() {
		if (this._instance) return;
		const appInfo = await this.backend.getAppInfo();
		this._instance = posthog.init(PUBLIC_POSTHOG_API_KEY, {
			api_host: 'https://eu.posthog.com',
			autocapture: false,
			disable_session_recording: true,
			capture_performance: false,
			request_batching: true,
			persistence: 'localStorage',
			on_xhr_error: (e) => {
				console.error('posthog error', e);
			}
		});
		posthog.register({
			appName: appInfo.name,
			appVersion: appInfo.version
		});
	}

	async setPostHogUser(params: { id: number; email?: string; name?: string }) {
		const { id, email, name } = params;
		const distinctId = `user_${id}`;
		this._instance?.identify(distinctId, {
			email,
			name
		});
		this.settingsService.updateTelemetryDistinctId(distinctId);
	}

	setAnonymousPostHogUser() {
		if (this._instance) {
			const distinctId = this._instance.get_distinct_id();
			this.settingsService.updateTelemetryDistinctId(distinctId);
		}
	}

	async resetPostHog() {
		this._instance?.capture('logout');
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
			this._instance?.unregister_for_session('repoDomain');
			this._instance?.unregister_for_session('repoHash');
		}
	}
}

export enum OnboardingEvent {
	ConfirmedAnalytics = 'onboarding_confirmed_analytics',
	AddLocalProject = 'onboarding_add_local_project',
	AddLocalProjectFailed = 'onboarding_add_local_project_failed',
	ClonedProject = 'onboarding_cloned_project',
	ClonedProjectFailed = 'onboarding_cloned_project_failed',
	ProjectSetupContinue = 'onboarding_project_setup_continue',
	SetTargetBranch = 'onboarding_set_target_branch',
	SetTargetBranchFailed = 'onboarding_set_target_branch_failed',
	SetProjectActive = 'onboarding_set_project_active',
	SetProjectActiveFailed = 'onboarding_set_project_active_failed',
	LoginGitButler = 'onboarding_login_gitbutler',
	CancelLoginGitButler = 'onboarding_cancel_login_gitbutler',
	GitHubInitiateOAuth = 'onboarding_github_initiate_oauth',
	GitHubOAuthFailed = 'onboarding_github_oauth_failed'
}
