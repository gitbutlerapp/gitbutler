<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import EditMode from '$components/EditMode.svelte';
	import { MODE_SERVICE, type EditModeMetadata } from '$lib/mode/modeService';
	import { inject } from '@gitbutler/shared/context';

	// TODO: Refactor so we don't need non-null assertion.
	const projectId = $derived(page.params.projectId!);
	const modeService = inject(MODE_SERVICE);

	const mode = modeService.mode;

	let editModeMetadata = $state<EditModeMetadata>();

	$effect(() => {
		if ($mode?.type === 'Edit') {
			editModeMetadata = $mode.subject;
		} else {
			goto(`/${projectId}`);
		}
	});
</script>

{#if editModeMetadata}
	<EditMode {projectId} {editModeMetadata} />
{/if}
