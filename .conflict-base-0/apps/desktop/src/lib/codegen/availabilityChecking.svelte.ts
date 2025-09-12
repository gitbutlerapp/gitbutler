import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
import { inject } from '@gitbutler/core/context';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { fromStore } from 'svelte/store';

export function useAvailabilityChecking() {
	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = fromStore(settingsService.appSettings);

	let claudeExecutable = $state('');
	let recheckedAvailability = $state<'recheck-failed' | 'recheck-succeeded'>();

	$effect(() => {
		if (settingsStore.current?.claude) {
			claudeExecutable = settingsStore.current.claude.executable;
		}
	});

	async function checkClaudeAvailability() {
		const recheck = await claudeCodeService.fetchCheckAvailable(undefined, { forceRefetch: true });
		if (recheck?.status === 'available') {
			recheckedAvailability = 'recheck-succeeded';
		} else {
			recheckedAvailability = 'recheck-failed';
		}
	}

	async function updateClaudeExecutable(value: string) {
		claudeExecutable = value;
		recheckedAvailability = undefined;
		await settingsService.updateClaude({ executable: value });
	}

	return {
		claudeExecutable: reactive(() => claudeExecutable),
		recheckedAvailability: reactive(() => recheckedAvailability),
		checkClaudeAvailability,
		updateClaudeExecutable
	};
}
