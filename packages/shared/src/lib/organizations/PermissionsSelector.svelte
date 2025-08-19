<script lang="ts">
	import { inject } from '$lib/context';
	import Loading from '$lib/network/Loading.svelte';
	import { PROJECT_SERVICE } from '$lib/organizations/projectService';
	import { getProjectByRepositoryId } from '$lib/organizations/projectsPreview.svelte';
	import { ShareLevel } from '$lib/permissions';
	import { ContextMenuItem, ContextMenuSection, DropdownButton } from '@gitbutler/ui';

	type Props = {
		repositoryId: string;
	};

	const { repositoryId }: Props = $props();

	const projectService = inject(PROJECT_SERVICE);

	const project = $derived(getProjectByRepositoryId(repositoryId));

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

	let dropDownButton = $state<DropdownButton>();
	let dropDownEnabled = $state(true);
</script>

<Loading loadable={project.current}>
	{#snippet children(project)}
		<DropdownButton bind:this={dropDownButton} loading={!dropDownEnabled} kind="outline">
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
		</DropdownButton>
	{/snippet}
</Loading>
