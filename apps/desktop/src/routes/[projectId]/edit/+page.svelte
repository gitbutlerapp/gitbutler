<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import EditMode from '$components/EditMode.svelte';
	import { ModeService, type EditModeMetadata } from '$lib/mode/modeService';
	import { Project } from '$lib/project/project';
	import { getContext } from '@gitbutler/shared/context';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';

	const projectId = $derived(page.params.projectId!);

	const modeService = getContext(ModeService);
	const project = getContext(Project);
	const mode = $derived(modeService.mode({ projectId }));

	let editModeMetadata = $state<EditModeMetadata>();

	$effect(() => {
		if (!isDefined(mode.current.data)) return;

		if (mode.current.data.type === 'Edit') {
			editModeMetadata = mode.current.data.subject;
		} else {
			goto(`/${project.id}`);
		}
	});
</script>

{#if editModeMetadata}
	<EditMode {editModeMetadata} />
{/if}
