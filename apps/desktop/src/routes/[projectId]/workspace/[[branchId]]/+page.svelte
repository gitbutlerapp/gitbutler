<script lang="ts">
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { getContext } from '@gitbutler/shared/context';
	import type { LayoutData } from '../../$types';
	import { goto } from '$app/navigation';

	const settingsService = getContext(SettingsService);
	const settingsStore = settingsService.appSettings;

	const { data }: { data: LayoutData } = $props();

	// Redirect to board if we have switched away from V3 feature.
	$effect(() => {
		if (!$settingsStore?.featureFlags.v3) {
			goto(`/${data.projectId}/board`);
		}
	});
</script>

<div>Workspace Page</div>
