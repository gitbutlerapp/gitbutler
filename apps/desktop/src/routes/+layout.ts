import { PromptService as AIPromptService } from '$lib/ai/promptService';
import { initAnalyticsIfEnabled } from '$lib/analytics/analytics';
import { EventContext } from '$lib/analytics/eventContext';
import { PostHogWrapper } from '$lib/analytics/posthog';
import createBackend from '$lib/backend';
import { loadAppSettings } from '$lib/config/appSettings';
import { SettingsService } from '$lib/config/appSettingsV2';
import { GitConfigService } from '$lib/config/gitConfigService';
import { FileService } from '$lib/files/fileService';
import { HooksService } from '$lib/hooks/hooksService';
import { PromptService } from '$lib/prompt/promptService';
import { RemotesService } from '$lib/remotes/remotesService';
import { TokenMemoryService } from '$lib/stores/tokenMemoryService';
import { UserService } from '$lib/user/userService';
import { HttpClient } from '@gitbutler/shared/network/httpClient';
import { UploadsService } from '@gitbutler/shared/uploads/uploadsService';
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
	const tokenMemoryService = new TokenMemoryService();
	const httpClient = new HttpClient(window.fetch, PUBLIC_API_BASE_URL, tokenMemoryService.token);
	const uploadsService = new UploadsService(httpClient);
	const backend = createBackend();
	const promptService = new PromptService(backend);

	const settingsService = new SettingsService(backend);

	const eventContext = new EventContext();
	// Awaited and will block initial render, but it is necessary in order to respect the user
	// settings on telemetry.
	const posthog = new PostHogWrapper(settingsService, eventContext);
	const appSettings = await loadAppSettings();
	initAnalyticsIfEnabled(appSettings, posthog);

	const gitConfig = new GitConfigService(backend);
	const remotesService = new RemotesService(backend);
	const aiPromptService = new AIPromptService();
	const fileService = new FileService(backend);
	const hooksService = new HooksService(backend);
	const userService = new UserService(backend, httpClient, tokenMemoryService, posthog);

	const homeDir = await backend.homeDirectory();

	// Await settings to prevent immediate reloads on initial render.
	await settingsService.refresh();

	return {
		homeDir,
		tokenMemoryService,
		appSettings,
		httpClient,
		promptService,
		userService,
		gitConfig,
		remotesService,
		aiPromptService,
		posthog,
		backend,
		fileService,
		hooksService,
		settingsService,
		uploadsService,
		eventContext
	};
};
