<script lang="ts">
	import type { Project } from '$lib/backend/projects';
	import IconButton from '$lib/components/IconButton.svelte';
	import SelectProjectItem from './SelectProjectItem.svelte';
	import * as events from '$lib/utils/events';
	import AccountLink from '$lib/components/AccountLink.svelte';
	import type { User } from '$lib/backend/cloud';

	export let projects: Project[] | undefined;
	export let user: User | undefined;
</script>

<div class="projects">
	<div class="projects__header">
		<span class="projects__title text-base-14 font-semibold">Projects</span>
	</div>
	<div class="projects__content">
		{#if projects && projects.length > 0}
			{#each projects || [] as project}
				<SelectProjectItem {project} />
			{/each}
		{:else}
			<pre>Go ahead and add your first project. :)</pre>
		{/if}
	</div>
	<div class="projects_footer">
		<IconButton icon="plus" on:click={() => events.emit('openNewProjectModal')}></IconButton>
		<AccountLink {user} />
	</div>
</div>

<style lang="postcss">
	.projects {
		display: flex;
		flex-direction: column;
		max-width: 640px;
		overflow-y: hidden;
		background: var(--clr-theme-container-light);
		border: 1px solid var(--clr-theme-container-outline-light);
		border-radius: var(--radius-m);
	}
	.projects__header {
		padding: var(--space-12);
		border-bottom: 1px solid var(--clr-theme-container-outline-light);
	}
	.projects__title {
		padding: var(--space-4) var(--space-6);
	}
	.projects__content {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
		padding: var(--space-16);
	}
	.projects_footer {
		display: flex;
		gap: var(--space-6);
		padding: var(--space-12);
		justify-content: space-between;
		border-top: 1px solid var(--clr-theme-container-outline-light);
	}
</style>
