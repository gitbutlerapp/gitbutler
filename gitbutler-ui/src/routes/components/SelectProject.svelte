<script lang="ts">
	import type { ProjectService } from '$lib/backend/projects';
	import IconButton from '$lib/components/IconButton.svelte';
	import SelectProjectItem from './SelectProjectItem.svelte';
	import AccountLink from '$lib/components/AccountLink.svelte';
	import type { User } from '$lib/backend/cloud';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';

	export let user: User | undefined;
	export let loading = false;
	export let projectService: ProjectService;

	$: projects$ = projectService.projects$;
</script>

<div class="projects card">
	<div class="card__header">
		<span class="card__title text-base-14 font-semibold">Projects</span>
	</div>
	<ScrollableContainer initiallyVisible>
		{#if $projects$?.length > 0}
			{#each $projects$ as project}
				<SelectProjectItem {project} />
			{/each}
		{:else}
			<pre class="empty-message">Go ahead and add your first project. :)</pre>
		{/if}
	</ScrollableContainer>
	<div class="card__footer">
		<IconButton
			icon="plus"
			{loading}
			on:click={async () => {
				loading = true;
				try {
					await projectService.addProject();
				} finally {
					loading = false;
				}
			}}
		></IconButton>
		<AccountLink {user} />
	</div>
</div>

<style lang="postcss">
	.projects {
		align-self: center;
		max-width: 640px;
		max-height: 65%;
	}

	.empty-message {
		padding: var(--space-12) var(--space-16);
	}
</style>
