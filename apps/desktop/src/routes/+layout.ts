import { PromptService as AIPromptService } from '$lib/ai/promptService';
import { AIService } from '$lib/ai/service';
import { initAnalyticsIfEnabled } from '$lib/analytics/analytics';
import { AuthService } from '$lib/backend/auth';
import { GitConfigService } from '$lib/backend/gitConfigService';
import { HttpClient } from '$lib/backend/httpClient';
import { ProjectService } from '$lib/backend/projects';
import { PromptService } from '$lib/backend/prompt';
import { Tauri } from '$lib/backend/tauri';
import { UpdaterService } from '$lib/backend/updater';
import { RemotesService } from '$lib/remotes/service';
import { RustSecretService } from '$lib/secrets/secretsService';
import { UserService } from '$lib/stores/user';
import { LineManagerFactory } from '@gitbutler/ui/commitLines/lineManager';
import { LineManagerFactory as StackingLineManagerFactory } from '@gitbutler/ui/commitLinesStacking/lineManager';
import lscache from 'lscache';
import type { LayoutLoad } from './$types';

// call on startup so we don't accumulate old items
lscache.flushExpired();

export const ssr = false;
export const prerender = false;
export const csr = true;

// eslint-disable-next-line
export const load: LayoutLoad = async () => {
	initAnalyticsIfEnabled();

	// TODO: Find a workaround to avoid this dynamic import
	// https://github.com/sveltejs/kit/issues/905
	const defaultPath = await (await import('@tauri-apps/api/path')).homeDir();

	const httpClient = new HttpClient();
	const authService = new AuthService();
	const projectService = new ProjectService(defaultPath, httpClient);
	const updaterService = new UpdaterService(new Tauri());
	const promptService = new PromptService();
	const userService = new UserService(httpClient);

	const gitConfig = new GitConfigService();
	const secretsService = new RustSecretService(gitConfig);
	const aiService = new AIService(gitConfig, secretsService, httpClient);
	const remotesService = new RemotesService();
	const aiPromptService = new AIPromptService();
	const lineManagerFactory = new LineManagerFactory();
	const stackingLineManagerFactory = new StackingLineManagerFactory();

	return {
		authService,
		cloud: httpClient,
		projectService,
		updaterService,
		promptService,
		userService,
		gitConfig,
		aiService,
		remotesService,
		aiPromptService,
		lineManagerFactory,
		stackingLineManagerFactory,
		secretsService
	};
};
