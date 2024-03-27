<script lang="ts">
	import ListItem from './ListItem.svelte';
	import ScrollableContainer from './ScrollableContainer.svelte';
	import { ProjectService } from '$lib/backend/projects';
	import { getContextByClass } from '$lib/utils/context';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	export let isNavCollapsed: boolean;

	const projectService = getContextByClass(ProjectService);
	const projects$ = projectService.projects$;

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
		{#if $projects$.length > 0}
			<ScrollableContainer maxHeight="20rem">
				<div class="popup__projects">
					{#each $projects$ as project}
						{@const selected = project.id == $page.params.projectId}
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
		z-index: 50;
		width: 100%;
		margin-top: var(--size-6);
		border-radius: var(--m, 6px);
		border: 1px solid var(--clr-theme-container-outline-light);
		background: var(--clr-theme-container-light);
		/* shadow/s */
		box-shadow: 0px 7px 14px 0px rgba(0, 0, 0, 0.1);
	}
	.popup__actions {
		padding: var(--size-8);
		border-top: 1px solid var(--clr-theme-scale-ntrl-70);
	}
	.popup__projects {
		display: flex;
		flex-direction: column;
		gap: var(--size-2);
		padding: var(--size-8);
	}

	/* MODIFIERS */
	.popup.collapsed {
		width: 240px;
	}
</style>
