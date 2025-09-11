<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import EditMode from '$components/EditMode.svelte';
	import { MODE_SERVICE, type EditModeMetadata } from '$lib/mode/modeService';
	import { inject } from '@gitbutler/core/context';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';

	// TODO: Refactor so we don't need non-null assertion.
	const projectId = $derived(page.params.projectId!);
	const modeService = inject(MODE_SERVICE);
	const mode = $derived(modeService.mode({ projectId }));

	let editModeMetadata = $state<EditModeMetadata>();

	$effect(() => {
		if (!isDefined(mode.current.data)) return;

		if (mode.current.data.type === 'Edit') {
			editModeMetadata = mode.current.data.subject;
		} else {
			goto(`/${projectId}`);
		}
	});
</script>

{#if editModeMetadata}
	<EditMode {projectId} {editModeMetadata} />
{/if}
