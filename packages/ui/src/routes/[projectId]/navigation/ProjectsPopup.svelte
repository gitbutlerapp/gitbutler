<script lang="ts">
	import type { Project } from '$lib/backend/projects';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { emit } from '$lib/utils/events';
	import ListItem from './ListItem.svelte';

	export let projects: Project[];
	let hidden = true;

	export function toggle() {
		hidden = !hidden;
		return !hidden;
	}

	export function hide() {
		hidden = true;
	}
</script>

{#if !hidden}
	<div class="popup">
		{#if projects.length > 0}
			<div class="popup__projects">
				{#each projects as project}
					{@const selected = project.id == $page.params.projectId}
					<ListItem
						{selected}
						icon={selected ? 'tick' : undefined}
						on:click={() => {
							hide();
							goto(`/${project.id}/board`);
						}}
					>
						{project.title}
					</ListItem>
				{/each}
			</div>
		{/if}
		<div class="popup__actions">
			<ListItem icon="plus" on:click={() => emit('openNewProjectModal')}>Add new project</ListItem>
		</div>
	</div>
{/if}

<style lang="postcss">
	.popup {
		position: absolute;
		top: 100%;
		z-index: 50;
		width: 100%;
		margin-top: var(--space-6);
		border-radius: var(--m, 6px);
		border: 1px solid var(--clr-theme-container-outline-light);
		background: var(--clr-theme-container-light);
		/* shadow/s */
		box-shadow: 0px 7px 14px 0px rgba(0, 0, 0, 0.1);
	}
	.popup__actions {
		padding: var(--space-8);
		border-top: 1px solid var(--clr-theme-scale-ntrl-70);
	}
	.popup__projects {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-8);
	}
</style>
