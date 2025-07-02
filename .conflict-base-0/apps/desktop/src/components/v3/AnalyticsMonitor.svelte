<!--
@component
This component keeps the analytics context up-to-date, i.e. the metadata
attached to posthog events.
-->
<script lang="ts">
	import { AnalyticsContext } from '$lib/analytics/analyticsContext';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { confettiEnabled } from '$lib/config/uiFeatureFlags';
	import { ProjectService } from '$lib/project/projectService';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContextStoreBySymbol, inject } from '@gitbutler/shared/context';
	import type { Writable } from 'svelte/store';

	const { projectId }: { projectId: string } = $props();

	const [uiState, analyticsContext, projectService, settingsService] = inject(
		UiState,
		AnalyticsContext,
		ProjectService,
		SettingsService
	);

	const globalState = uiState.global;
	const projectState = $derived(uiState.project(projectId));

	const settings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

	$effect(() => {
		analyticsContext.update({
			showActions: projectState.showActions.current,
			exclusiveAction: projectState.exclusiveAction.current?.type
		});
	});

	$effect(() => {
		analyticsContext.update({
			rulerCount: globalState.rulerCountValue.current,
			useRuler: globalState.useRuler.current,
			wrapTextByRuler: globalState.wrapTextByRuler.current
		});
	});

	$effect(() => {
		analyticsContext.update({
			zoom: $settings.zoom,
			theme: $settings.theme,
			tabSize: $settings.tabSize,
			defaultCodeEditor: $settings.defaultCodeEditor.schemeIdentifer,
			aiSummariesEnabled: $settings.aiSummariesEnabled,
			diffLigatures: $settings.diffLigatures
		});
	});

	$effect(() => {
		analyticsContext.update({
			forcePushAllowed: $projectService?.ok_with_force_push,
			gitAuthType: $projectService?.gitAuthType()
		});
	});

	$effect(() => {
		analyticsContext.update({
			v3: $settingsService?.featureFlags.v3,
			butlerActions: $settingsService?.featureFlags.actions,
			ws3: $settingsService?.featureFlags.ws3
		});
	});

	$effect(() => {
		analyticsContext.update({ confetti: $confettiEnabled });
	});
</script>

<style lang="postcss">
</style>
