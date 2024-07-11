import { PromptService as AIPromptService } from '$lib/ai/promptService';
import { AIService } from '$lib/ai/service';
import { initAnalyticsIfEnabled } from '$lib/analytics/analytics';
import { AuthService } from '$lib/backend/auth';
import { GitConfigService } from '$lib/backend/gitConfigService';
import { HttpClient } from '$lib/backend/httpClient';
import { ProjectService } from '$lib/backend/projects';
import { PromptService } from '$lib/backend/prompt';
import { UpdaterService } from '$lib/backend/updater';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import { RemotesService } from '$lib/remotes/service';
import { RustSecretService } from '$lib/secrets/secretsService';
import { UserService } from '$lib/stores/user';
import { mockTauri } from '$lib/testing/index';
import { LineManagerFactory } from '@gitbutler/ui/CommitLines/lineManager';
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
	// It feels we should split GitHubService into unauthenticated/authenticated parts so we can
	// declare project metrics in `/[projectId]/layout.ts` instead of here. The current solution
	// requires the `projectId` field to be mutable, and be updated when the user loads a new
	// project.
	const projectMetrics = new ProjectMetrics();

	const gitConfig = new GitConfigService();
	const secretsService = new RustSecretService(gitConfig);
	const aiService = new AIService(gitConfig, secretsService, httpClient);
	const remotesService = new RemotesService();
	const aiPromptService = new AIPromptService();
	const lineManagerFactory = new LineManagerFactory();

	return {
		authService,
		cloud: httpClient,
		projectService,
		updaterService,
		promptService,
		userService,
		remoteUrl$,
		gitConfig,
		aiService,
		remotesService,
		aiPromptService,
		lineManagerFactory,
		secretsService,
		projectMetrics
	};
}
