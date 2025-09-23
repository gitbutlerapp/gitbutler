import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
import { inject } from '@gitbutler/core/context';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { fromStore } from 'svelte/store';

// Global shared state for availability checking
let globalRecheckedAvailability = $state<'recheck-failed' | 'recheck-succeeded'>();

export function useAvailabilityChecking() {
	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = fromStore(settingsService.appSettings);

	let claudeExecutable = $state('');

	$effect(() => {
		if (settingsStore.current?.claude) {
			claudeExecutable = settingsStore.current.claude.executable;
		}
	});

	async function checkClaudeAvailability() {
		const recheck = await claudeCodeService.fetchCheckAvailable(undefined, { forceRefetch: true });
		if (recheck?.status === 'available') {
			globalRecheckedAvailability = 'recheck-succeeded';
		} else {
			globalRecheckedAvailability = 'recheck-failed';
		}
	}

	async function updateClaudeExecutable(value: string) {
		claudeExecutable = value;
		globalRecheckedAvailability = undefined;
		await settingsService.updateClaude({ executable: value });
	}

	return {
		claudeExecutable: reactive(() => claudeExecutable),
		recheckedAvailability: reactive(() => globalRecheckedAvailability),
		checkClaudeAvailability,
		updateClaudeExecutable
	};
}
