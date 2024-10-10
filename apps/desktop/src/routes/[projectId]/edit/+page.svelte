<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import EditMode from '$lib/components/EditMode.svelte';
	import { ModeService, type EditModeMetadata } from '$lib/modes/service';
	import { getContext } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';

	const modeService = getContext(ModeService);
	const project = getContext(Project);
	const mode = modeService.mode;

	let editModeMetadata = $state<EditModeMetadata>();

	$effect(() => {
		if ($mode?.type === 'Edit') {
			editModeMetadata = $mode.subject;
		} else {
			goto(`/${project.id}/board`);
		}
	});
</script>

{#if editModeMetadata}
	<EditMode {editModeMetadata} />
{/if}
