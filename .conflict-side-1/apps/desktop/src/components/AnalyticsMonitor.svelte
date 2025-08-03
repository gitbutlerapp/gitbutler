<!--
@component
This component keeps the analytics context up-to-date, i.e. the metadata
attached to posthog events.
-->
<script lang="ts">
	import { EVENT_CONTEXT } from '$lib/analytics/eventContext';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { gitAuthType } from '$lib/project/project';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';

	const { projectId }: { projectId: string } = $props();

	const uiState = inject(UI_STATE);
	const eventContext = inject(EVENT_CONTEXT);
	const projectsService = inject(PROJECTS_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);

	const projectResult = $derived(projectsService.getProject(projectId));
	const globalState = uiState.global;
	const projectState = $derived(uiState.project(projectId));

	const settings = inject(SETTINGS);

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
		const project = projectResult.current.data;
		if (project) {
			eventContext.update({
				forcePushAllowed: project.ok_with_force_push,
				gitAuthType: gitAuthType(project.preferred_key)
			});
		}
	});

	$effect(() => {
		eventContext.update({
			v3: true,
			butlerActions: $settingsService?.featureFlags.actions,
			ws3: $settingsService?.featureFlags.ws3
		});
	});
</script>

<style lang="postcss">
</style>
