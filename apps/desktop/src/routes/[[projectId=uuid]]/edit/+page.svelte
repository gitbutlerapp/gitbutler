<script lang="ts">
	import { goto } from "$app/navigation";
	import EditCommitPanel from "$components/workspace/EditCommitPanel.svelte";
	import { MODE_SERVICE, type EditModeMetadata } from "$lib/mode/modeService";
	import { projectPath } from "$lib/routes/routes.svelte";
	import { inject } from "@gitbutler/core/context";
	import { isDefined } from "@gitbutler/ui/utils/typeguards";

	import type { PageData } from "./$types";

	const { data }: { data: PageData } = $props();
	const projectId = $derived(data.projectId);
	const modeService = inject(MODE_SERVICE);
	const mode = $derived(modeService.mode(projectId));

	let editModeMetadata = $state<EditModeMetadata>();

	$effect(() => {
		if (!isDefined(mode.response)) return;

		if (mode.response.type === "Edit") {
			editModeMetadata = mode.response.subject;
		} else {
			goto(projectPath(projectId));
		}
	});
</script>

{#if editModeMetadata}
	<EditCommitPanel {projectId} {editModeMetadata} />
{/if}
