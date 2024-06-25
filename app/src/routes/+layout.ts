import { PromptService as AIPromptService } from '$lib/ai/promptService';
import { AIService } from '$lib/ai/service';
import { initAnalyticsIfEnabled } from '$lib/analytics/analytics';
import { AuthService } from '$lib/backend/auth';
import { GitConfigService } from '$lib/backend/gitConfigService';
import { HttpClient } from '$lib/backend/httpClient';
import { ProjectService } from '$lib/backend/projects';
import { PromptService } from '$lib/backend/prompt';
import { UpdaterService } from '$lib/backend/updater';
import { LineManagerFactory } from '$lib/commitLines/lineManager';
import { GitHubService } from '$lib/github/service';
import { RemotesService } from '$lib/remotes/service';
import { UserService } from '$lib/stores/user';
import { mockTauri } from '$lib/testing/index';
import lscache from 'lscache';
import { BehaviorSubject, config } from 'rxjs';
import { env } from '$env/dynamic/public';

// call on startup so we don't accumulate old items
lscache.flushExpired();

// https://rxjs.dev/api/index/interface/GlobalConfig#properties
config.onUnhandledError = (err) => console.warn(err);

export const ssr = false;
export const prerender = false;
export const csr = true;

export async function load() {
	// Mock Tauri API during E2E tests
	if (env.PUBLIC_TESTING) {
		mockTauri();
	}
	initAnalyticsIfEnabled();

	// TODO: Find a workaround to avoid this dynamic import
	// https://github.com/sveltejs/kit/issues/905
	const defaultPath = await (await import('@tauri-apps/api/path')).homeDir();

	const httpClient = new HttpClient();
	const authService = new AuthService();
	const projectService = new ProjectService(defaultPath, httpClient);
	const updaterService = new UpdaterService();
	const promptService = new PromptService();
	const userService = new UserService(httpClient);

	// We're declaring a remoteUrl$ observable here that is written to by `BaseBranchService`. This
	// is a bit awkard, but `GitHubService` needs to be available at the root scoped layout.ts, such
	// that we can perform actions related to GitHub that do not depend on repo information.
	//     We should evaluate whether or not to split this service into two separate services. That
	// way we would not need `remoteUrl$` for the non-repo service, and therefore the other one
	// could easily get an observable of the remote url from `BaseBranchService`.
	const remoteUrl$ = new BehaviorSubject<string | undefined>(undefined);
	const githubService = new GitHubService(userService.accessToken$, remoteUrl$);

	const gitConfig = new GitConfigService();
	const aiService = new AIService(gitConfig, httpClient);
	const remotesService = new RemotesService();
	const aiPromptService = new AIPromptService();
	const lineManagerFactory = new LineManagerFactory();

	return {
		authService,
		cloud: httpClient,
		githubService,
		projectService,
		updaterService,
		promptService,
		userService,
		// These observables are provided for convenience
		remoteUrl$,
		gitConfig,
		aiService,
		remotesService,
		aiPromptService,
		lineManagerFactory
	};
}
