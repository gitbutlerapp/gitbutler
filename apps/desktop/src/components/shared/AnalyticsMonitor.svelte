<!--
@component
This component keeps the analytics context up-to-date, i.e. the metadata
attached to posthog events.
-->
<script lang="ts">
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { EVENT_CONTEXT } from "$lib/telemetry/eventContext";
	import { inject } from "@gitbutler/core/context";

	const { projectId }: { projectId: string } = $props();

	const uiState = inject(UI_STATE);
	const eventContext = inject(EVENT_CONTEXT);

	const globalState = uiState.global;
	const projectState = $derived(uiState.project(projectId));

	$effect(() => {
		eventContext.update({
			exclusiveAction: projectState.exclusiveAction.current?.type,
		});
	});

	$effect(() => {
		eventContext.update({
			rulerCount: globalState.rulerCountValue.current,
			useRuler: globalState.useRuler.current,
		});
	});

	$effect(() => {
		eventContext.update({
			zoom: globalState.zoom.current,
			theme: globalState.theme.current,
			tabSize: globalState.tabSize.current,
			defaultCodeEditor: globalState.defaultCodeEditor.current.schemeIdentifer,
			aiSummariesEnabled: globalState.aiSummariesEnabled.current,
			diffLigatures: globalState.diffLigatures.current,
		});
	});

	$effect(() => {
		eventContext.update({
			v3: true,
		});
	});
</script>

<style lang="postcss">
</style>
