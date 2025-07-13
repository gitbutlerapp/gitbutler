<!--
@component
This component keeps the analytics context up-to-date, i.e. the metadata
attached to posthog events.
-->
<script lang="ts">
	import { EventContext } from '$lib/analytics/eventContext';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { ProjectService } from '$lib/project/projectService';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContextStoreBySymbol, inject } from '@gitbutler/shared/context';
	import type { Writable } from 'svelte/store';

	const { projectId }: { projectId: string } = $props();

	const [uiState, eventContext, projectService, settingsService] = inject(
		UiState,
		EventContext,
		ProjectService,
		SettingsService
	);

	const globalState = uiState.global;
	const projectState = $derived(uiState.project(projectId));

	const settings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

	$effect(() => {
		eventContext.update({
			showActions: projectState.showActions.current,
			exclusiveAction: projectState.exclusiveAction.current?.type
		});
	});

	$effect(() => {
		eventContext.update({
			rulerCount: globalState.rulerCountValue.current,
			useRuler: globalState.useRuler.current
		});
	});

	$effect(() => {
		eventContext.update({
			zoom: $settings.zoom,
			theme: $settings.theme,
			tabSize: $settings.tabSize,
			defaultCodeEditor: $settings.defaultCodeEditor.schemeIdentifer,
			aiSummariesEnabled: $settings.aiSummariesEnabled,
			diffLigatures: $settings.diffLigatures
		});
	});

	$effect(() => {
		eventContext.update({
			forcePushAllowed: $projectService?.ok_with_force_push,
			gitAuthType: $projectService?.gitAuthType()
		});
	});

	$effect(() => {
		eventContext.update({
			v3: $settingsService?.featureFlags.v3,
			butlerActions: $settingsService?.featureFlags.actions,
			ws3: $settingsService?.featureFlags.ws3
		});
	});
</script>

<style lang="postcss">
</style>
