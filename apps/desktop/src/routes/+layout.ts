import { PromptService as AIPromptService } from '$lib/ai/promptService';
import { AIService } from '$lib/ai/service';
import { initAnalyticsIfEnabled } from '$lib/analytics/analytics';
import { PostHogWrapper } from '$lib/analytics/posthog';
import { CommandService } from '$lib/backend/ipc';
import { PromptService } from '$lib/backend/prompt';
import { Tauri } from '$lib/backend/tauri';
import { loadAppSettings } from '$lib/config/appSettings';
import { GitConfigService } from '$lib/config/gitConfigService';
import { FileService } from '$lib/files/fileService';
import { HooksService } from '$lib/hooks/hooksService';
import { ProjectsService } from '$lib/project/projectsService';
import { RemotesService } from '$lib/remotes/remotesService';
import { RustSecretService } from '$lib/secrets/secretsService';
import { TokenMemoryService } from '$lib/stores/tokenMemoryService';
import { UpdaterService } from '$lib/updater/updater';
import { UserService } from '$lib/user/userService';
import { HttpClient } from '@gitbutler/shared/network/httpClient';
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
	const tauri = new Tauri();
	const updaterService = new UpdaterService(tauri, posthog);
	const promptService = new PromptService();

	const userService = new UserService(httpClient, tokenMemoryService, posthog);

	const projectsService = new ProjectsService(defaultPath, httpClient);

	const gitConfig = new GitConfigService(tauri);
	const secretsService = new RustSecretService(gitConfig);
	const aiService = new AIService(gitConfig, secretsService, httpClient, tokenMemoryService);
	const remotesService = new RemotesService();
	const aiPromptService = new AIPromptService();
	const lineManagerFactory = new LineManagerFactory();
	const stackingLineManagerFactory = new StackingLineManagerFactory();
	const fileService = new FileService(tauri);
	const hooksService = new HooksService(tauri);

	return {
		commandService,
		tokenMemoryService,
		appSettings,
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
		fileService,
		hooksService
	};
};
