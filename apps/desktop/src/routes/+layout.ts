import { PromptService as AIPromptService } from '$lib/ai/promptService';
import { AIService } from '$lib/ai/service';
import { initAnalyticsIfEnabled } from '$lib/analytics/analytics';
import { PostHogWrapper } from '$lib/analytics/posthog';
import { AuthService } from '$lib/backend/auth';
import { GitConfigService } from '$lib/backend/gitConfigService';
import { CommandService } from '$lib/backend/ipc';
import { ProjectsService } from '$lib/backend/projects';
import { PromptService } from '$lib/backend/prompt';
import { Tauri } from '$lib/backend/tauri';
import { UpdaterService } from '$lib/backend/updater';
import { loadAppSettings } from '$lib/config/appSettings';
import { FileService } from '$lib/files/fileService';
import { RemotesService } from '$lib/remotes/service';
import { RustSecretService } from '$lib/secrets/secretsService';
import { TokenMemoryService } from '$lib/stores/tokenMemoryService';
import { UserService } from '$lib/stores/user';
import { HttpClient } from '@gitbutler/shared/httpClient';
import { LineManagerFactory } from '@gitbutler/ui/commitLines/lineManager';
import { LineManagerFactory as StackingLineManagerFactory } from '@gitbutler/ui/commitLines/lineManager';
import lscache from 'lscache';
import type { LayoutLoad } from './$types';
import { PUBLIC_API_BASE_URL } from '$env/static/public';

// call on startup so we don't accumulate old items
lscache.flushExpired();

export const ssr = false;
export const prerender = false;
export const csr = true;

// eslint-disable-next-line
export const load: LayoutLoad = async () => {
	// Awaited and will block initial render, but it is necessary in order to respect the user
	// settings on telemetry.
	const posthog = new PostHogWrapper();
	const appSettings = await loadAppSettings();
	initAnalyticsIfEnabled(appSettings, posthog);

	// TODO: Find a workaround to avoid this dynamic import
	// https://github.com/sveltejs/kit/issues/905
	const defaultPath = await (await import('@tauri-apps/api/path')).homeDir();

	const commandService = new CommandService();

	const tokenMemoryService = new TokenMemoryService();
	const httpClient = new HttpClient(window.fetch, PUBLIC_API_BASE_URL, tokenMemoryService.token);
	const authService = new AuthService();
	const tauri = new Tauri();
	const updaterService = new UpdaterService(tauri, posthog);
	const promptService = new PromptService();

	const userService = new UserService(httpClient, tokenMemoryService, posthog);

	const projectsService = new ProjectsService(defaultPath, httpClient);

	const gitConfig = new GitConfigService();
	const secretsService = new RustSecretService(gitConfig);
	const aiService = new AIService(gitConfig, secretsService, httpClient, tokenMemoryService);
	const remotesService = new RemotesService();
	const aiPromptService = new AIPromptService();
	const lineManagerFactory = new LineManagerFactory();
	const stackingLineManagerFactory = new StackingLineManagerFactory();
	const fileService = new FileService(tauri);

	return {
		commandService,
		tokenMemoryService,
		appSettings,
		authService,
		cloud: httpClient,
		projectsService,
		updaterService,
		promptService,
		userService,
		gitConfig,
		aiService,
		remotesService,
		aiPromptService,
		lineManagerFactory,
		stackingLineManagerFactory,
		secretsService,
		posthog,
		tauri,
		fileService
	};
};
