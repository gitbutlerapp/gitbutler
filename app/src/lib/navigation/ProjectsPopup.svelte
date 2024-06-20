<script lang="ts">
	import { ProjectService } from '$lib/backend/projects';
	import ListItem from '$lib/components/ListItem.svelte';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';
	import { getContext } from '$lib/utils/context';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	export let isNavCollapsed: boolean;

	const projectService = getContext(ProjectService);
	const projects = projectService.projects;

	let hidden = true;
	let loading = false;

	export function toggle() {
		hidden = !hidden;
		return !hidden;
	}

	export function hide() {
		hidden = true;
	}
</script>

{#if !hidden}
	<div class="popup" class:collapsed={isNavCollapsed}>
		{#if $projects.length > 0}
			<ScrollableContainer maxHeight="20rem">
				<div class="popup__projects">
					{#each $projects as project}
						<!-- eslint-disable-next-line svelte/valid-compile -->
						{@const selected = project.id === $page.params.projectId}
						<ListItem
							{selected}
							icon={selected ? 'tick' : undefined}
							on:click={() => {
								hide();
								projectService.setLastOpenedProject(project.id);
								goto(`/${project.id}/board`);
							}}
						>
							{project.title}
						</ListItem>
					{/each}
				</div>
			</ScrollableContainer>
		{/if}
		<div class="popup__actions">
			<ListItem
				icon="plus"
				{loading}
				on:click={async () => {
					loading = true;
					try {
						await projectService.addProject();
					} finally {
						loading = false;
					}
				}}>Add new project</ListItem
			>
		</div>
	</div>
{/if}

<style lang="postcss">
	.popup {
		position: absolute;
		top: 100%;
		z-index: var(--z-floating);
		width: 100%;
		margin-top: 6px;
		border-radius: var(--m, 6px);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
		/* shadow/s */
		box-shadow: 0px 7px 14px 0px rgba(0, 0, 0, 0.1);
	}
	.popup__actions {
		padding: 8px;
		border-top: 1px solid var(--clr-scale-ntrl-70);
	}
	.popup__projects {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 8px;
	}

	/* MODIFIERS */
	.popup.collapsed {
		width: 240px;
	}
</style>
