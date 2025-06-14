import { PostHogWrapper } from '$lib/analytics/posthog';
import { isTauriCommandError, type TauriCommandError } from '$lib/backend/ipc';
import { Tauri } from '$lib/backend/tauri';
import { SettingsService } from '$lib/config/appSettingsV2';
import { confettiEnabled } from '$lib/config/uiFeatureFlags';
import { isErrorlike } from '@gitbutler/ui/utils/typeguards';
import { type BaseQueryApi, type QueryReturnValue } from '@reduxjs/toolkit/query';
import { get, type Readable } from 'svelte/store';
import type { Project } from '$lib/project/project';
import type { Settings } from '$lib/settings/userSettings';

export type TauriBaseQueryFn = typeof tauriBaseQuery;

export async function tauriBaseQuery(
	args: ApiArgs,
	api: BaseQueryApi
): Promise<QueryReturnValue<unknown, TauriCommandError, undefined>> {
	if (!hasTauriExtra(api.extra)) {
		return {
			error: { name: 'Failed to execute Tauri query', message: 'Redux dependency Tauri not found!' }
		};
	}

	const posthog = hasPosthogExtra(api.extra) ? api.extra.posthog : undefined;
	const settingsService = hasSettingsExtra(api.extra) ? api.extra.settingsService : undefined;
	const userSettings = hasUserSettingsExtra(api.extra) ? get(api.extra.userSettings) : undefined;
	const appSettings = settingsService?.appSettings;
	const project = hasProjectExtra(api.extra) ? get(api.extra.project) : undefined;

	const v3 = appSettings ? get(appSettings)?.featureFlags.v3 : false;
	const butlerActions = appSettings ? get(appSettings)?.featureFlags.actions : false;
	const confetti = get(confettiEnabled);

	const someUserSettings = userSettings
		? {
				zoom: userSettings.zoom,
				theme: userSettings.theme,
				tabSize: userSettings?.tabSize,
				defaultCodeEditor: userSettings.defaultCodeEditor.schemeIdentifer,
				aiSummariesEnabled: userSettings.aiSummariesEnabled,
				diffLigatures: userSettings.diffLigatures,
				forcePushAllowed: project?.ok_with_force_push,
				gitAuthType: project?.gitAuthType()
			}
		: {};
	const settingsSnapshot = {
		...someUserSettings,
		v3,
		confetti,
		butlerActions
	};

	const startTime = Date.now();
	try {
		const result = { data: await api.extra.tauri.invoke(args.command, args.params) };
		const durationMs = Date.now() - startTime;
		posthog?.capture('tauri_command', {
			...settingsSnapshot,
			command: args.command,
			durationMs,
			failure: false
		});
		if (posthog && args.actionName) {
			posthog.capture(`${args.actionName} Successful`, {
				...settingsSnapshot,
				durationMs
			});
		}
		return result;
	} catch (error: unknown) {
		const durationMs = Date.now() - startTime;
		posthog?.capture('tauri_command', {
			...settingsSnapshot,
			command: args.command,
			durationMs,
			failure: true
		});
		if (posthog && args.actionName) {
			posthog.capture(`${args.actionName} Failed`, { error, ...settingsSnapshot });
		}

		const name = `API error: ${args.actionName} (${args.command})`;
		if (isTauriCommandError(error)) {
			const newMessage =
				`command: ${args.command}\nparams: ${JSON.stringify(args.params)})\n\n` + error.message;
			return { error: { name, message: newMessage, code: error.code } };
		}

		if (isErrorlike(error)) {
			return { error: { name, message: error.message } };
		}

		return { error: { name, message: String(error) } };
	}
}

type ApiArgs = {
	command: string;
	params: Record<string, unknown>;
	actionName?: string;
};

/**
 * Typeguard for accessing injected Tauri dependency safely.
 */
export function hasTauriExtra(extra: unknown): extra is {
	tauri: Tauri;
} {
	return (
		!!extra &&
		typeof extra === 'object' &&
		extra !== null &&
		'tauri' in extra &&
		extra.tauri instanceof Tauri
	);
}

export function hasPosthogExtra(extra: unknown): extra is {
	posthog: PostHogWrapper;
} {
	return (
		!!extra &&
		typeof extra === 'object' &&
		extra !== null &&
		'posthog' in extra &&
		extra.posthog instanceof PostHogWrapper
	);
}

export function hasSettingsExtra(extra: unknown): extra is {
	settingsService: SettingsService;
} {
	return (
		!!extra &&
		typeof extra === 'object' &&
		extra !== null &&
		'settingsService' in extra &&
		extra.settingsService instanceof SettingsService
	);
}

export function hasUserSettingsExtra(extra: unknown): extra is {
	userSettings: Readable<Settings>;
} {
	return !!extra && typeof extra === 'object' && extra !== null && 'userSettings' in extra;
}

export function hasProjectExtra(extra: unknown): extra is {
	project: Readable<Project>;
} {
	return !!extra && typeof extra === 'object' && extra !== null && 'project' in extra;
}
