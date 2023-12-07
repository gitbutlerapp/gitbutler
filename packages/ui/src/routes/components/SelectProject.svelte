<script lang="ts">
	import type { Project } from '$lib/backend/projects';
	import IconButton from '$lib/components/IconButton.svelte';
	import SelectProjectItem from './SelectProjectItem.svelte';
	import * as events from '$lib/utils/events';
	import AccountLink from '$lib/components/AccountLink.svelte';
	import type { User } from '$lib/backend/cloud';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';

	export let projects: Project[] | undefined;
	export let user: User | undefined;
</script>

<div class="projects card">
	<div class="card__header">
		<span class="card__title text-base-14 font-semibold">Projects</span>
	</div>
	<ScrollableContainer initiallyVisible>
		{#if projects && projects.length > 0}
			{#each projects || [] as project}
				<SelectProjectItem {project} />
			{/each}
		{:else}
			<pre>Go ahead and add your first project. :)</pre>
		{/if}
	</ScrollableContainer>
	<div class="card__footer">
		<IconButton icon="plus" on:click={() => events.emit('openNewProjectModal')}></IconButton>
		<AccountLink {user} />
	</div>
</div>

<style lang="postcss">
	.projects {
		align-self: center;
		max-width: 640px;
		max-height: 65%;
	}
</style>
