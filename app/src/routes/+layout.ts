import { AIService } from '$lib/ai/service';
import { initPostHog } from '$lib/analytics/posthog';
import { initSentry } from '$lib/analytics/sentry';
import { AuthService } from '$lib/backend/auth';
import { CloudClient } from '$lib/backend/cloud';
import { GitConfigService } from '$lib/backend/gitConfigService';
import { ProjectService } from '$lib/backend/projects';
import { PromptService } from '$lib/backend/prompt';
import { UpdaterService } from '$lib/backend/updater';
import { appMetricsEnabled, appErrorReportingEnabled } from '$lib/config/appSettings';
import { GitHubService } from '$lib/github/service';
import { UserService } from '$lib/stores/user';
import lscache from 'lscache';
import { BehaviorSubject, config } from 'rxjs';

// call on startup so we don't accumulate old items
lscache.flushExpired();

// https://rxjs.dev/api/index/interface/GlobalConfig#properties
config.onUnhandledError = (err) => console.warn(err);

export const ssr = false;
export const prerender = false;
export const csr = true;

export async function load() {
	appErrorReportingEnabled()
		.onDisk()
		.then((enabled) => {
			if (enabled) initSentry();
		});
	appMetricsEnabled()
		.onDisk()
		.then((enabled) => {
			if (enabled) initPostHog();
		});

	// TODO: Find a workaround to avoid this dynamic import
	// https://github.com/sveltejs/kit/issues/905
	const defaultPath = await (await import('@tauri-apps/api/path')).homeDir();

	const cloud = new CloudClient();
	const authService = new AuthService();
	const projectService = new ProjectService(defaultPath);
	const updaterService = new UpdaterService();
	const promptService = new PromptService();
	const userService = new UserService(cloud);

	// We're declaring a remoteUrl$ observable here that is written to by `BaseBranchService`. This
	// is a bit awkard, but `GitHubService` needs to be available at the root scoped layout.ts, such
	// that we can perform actions related to GitHub that do not depend on repo information.
	//     We should evaluate whether or not to split this service into two separate services. That
	// way we would not need `remoteUrl$` for the non-repo service, and therefore the other one
	// could easily get an observable of the remote url from `BaseBranchService`.
	const remoteUrl$ = new BehaviorSubject<string | undefined>(undefined);
	const githubService = new GitHubService(userService.accessToken$, remoteUrl$);

	const gitConfig = new GitConfigService();
	const aiService = new AIService(gitConfig, cloud);

	return {
		authService,
		cloud,
		githubService,
		projectService,
		updaterService,
		promptService,
		userService,
		// These observables are provided for convenience
		remoteUrl$,
		gitConfig,
		aiService
	};
}
