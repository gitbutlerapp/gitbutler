import { PromptService as AIPromptService } from '$lib/ai/promptService';
import { initAnalyticsIfEnabled } from '$lib/analytics/analytics';
import { EventContext } from '$lib/analytics/eventContext';
import { PostHogWrapper } from '$lib/analytics/posthog';
import { Tauri } from '$lib/backend/tauri';
import { loadAppSettings } from '$lib/config/appSettings';
import { SettingsService } from '$lib/config/appSettingsV2';
import { GitConfigService } from '$lib/config/gitConfigService';
import { FileService } from '$lib/files/fileService';
import { HooksService } from '$lib/hooks/hooksService';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
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
	// TODO: Find a workaround to avoid this dynamic import
	// https://github.com/sveltejs/kit/issues/905
	const homeDir = await (await import('@tauri-apps/api/path')).homeDir();

	const tokenMemoryService = new TokenMemoryService();
	const httpClient = new HttpClient(window.fetch, PUBLIC_API_BASE_URL, tokenMemoryService.token);
	const tauri = new Tauri();
	const promptService = new PromptService();
	const uploadsService = new UploadsService(httpClient);

	const settingsService = new SettingsService(tauri);

	const eventContext = new EventContext();
	// Awaited and will block initial render, but it is necessary in order to respect the user
	// settings on telemetry.
	const posthog = new PostHogWrapper(settingsService, eventContext);
	const appSettings = await loadAppSettings();
	initAnalyticsIfEnabled(appSettings, posthog);

	const gitConfig = new GitConfigService(tauri);
	const remotesService = new RemotesService();
	const aiPromptService = new AIPromptService();
	const fileService = new FileService(tauri);
	const hooksService = new HooksService(tauri);
	const projectMetrics = new ProjectMetrics();
	const userService = new UserService(httpClient, tokenMemoryService, posthog);

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
		tauri,
		fileService,
		hooksService,
		settingsService,
		projectMetrics,
		uploadsService,
		eventContext
	};
};
