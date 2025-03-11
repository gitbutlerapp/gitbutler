<script lang="ts">
	import { getContext } from '$lib/context';
	import Loading from '$lib/network/Loading.svelte';
	import { ProjectService } from '$lib/organizations/projectService';
	import { getProjectByRepositoryId } from '$lib/organizations/projectsPreview.svelte';
	import { ShareLevel } from '$lib/permissions';
	import { AppState } from '$lib/redux/store.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';

	type Props = {
		repositoryId: string;
	};

	const { repositoryId }: Props = $props();

	const appState = getContext(AppState);
	const projectService = getContext(ProjectService);

	const project = $derived(getProjectByRepositoryId(appState, projectService, repositoryId));

	const options = [
		{
			label: 'Private',
			key: ShareLevel.Private
		},
		{
			label: 'Unlisted',
			key: ShareLevel.Unlisted
		},
		{
			label: 'Public',
			key: ShareLevel.Public
		}
	];

	async function updatePermission(shareLevel: ShareLevel) {
		dropDownEnabled = false;
		try {
			await projectService.updateProject(repositoryId, { shareLevel });
		} finally {
			dropDownEnabled = true;
			dropDownButton?.close();
		}
	}

	let dropDownButton = $state<DropDownButton>();
	let dropDownEnabled = $state(true);
</script>

<Loading loadable={project.current}>
	{#snippet children(project)}
		<DropDownButton bind:this={dropDownButton} loading={!dropDownEnabled} kind="outline">
			{options.find((option) => option.key === project.permissions.shareLevel)?.label}

			{#snippet contextMenuSlot()}
				<ContextMenuSection>
					{#each options as option}
						<ContextMenuItem
							label={option.label}
							disabled={option.key === project.permissions.shareLevel}
							onclick={() => updatePermission(option.key)}
						/>
					{/each}
				</ContextMenuSection>
			{/snippet}
		</DropDownButton>
	{/snippet}
</Loading>
