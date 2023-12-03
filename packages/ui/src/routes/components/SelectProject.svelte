<script lang="ts">
	import type { Project } from '$lib/backend/projects';
	import IconButton from '$lib/components/IconButton.svelte';
	import SelectProjectItem from './SelectProjectItem.svelte';
	import * as events from '$lib/utils/events';
	import AccountLink from '$lib/components/AccountLink.svelte';
	import type { User } from '$lib/backend/cloud';
	import Scrollbar from '$lib/components/Scrollbar.svelte';

	export let projects: Project[] | undefined;
	export let user: User | undefined;

	let viewport: HTMLElement | undefined;
	let contents: HTMLElement | undefined;
</script>

<div class="projects card">
	<div class="card__header">
		<span class="card__title text-base-14 font-semibold">Projects</span>
	</div>
	<div class="scroll-wrapper">
		<div bind:this={viewport} class="viewport hide-native-scrollbar">
			<div bind:this={contents} class="card__content">
				{#if projects && projects.length > 0}
					{#each projects || [] as project}
						<SelectProjectItem {project} />
					{/each}
				{:else}
					<pre>Go ahead and add your first project. :)</pre>
				{/if}
			</div>
		</div>
		<Scrollbar {viewport} {contents} alwaysVisible thickness="0.4rem" opacity="0.1" />
	</div>
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
	.scroll-wrapper {
		display: flex;
		flex-direction: column;
		position: relative;
		overflow-y: hidden;
	}
	.viewport {
		height: 100%;
		overflow-y: scroll;
		overscroll-behavior: none;
	}
	.card__content {
		gap: var(--space-6);
	}
</style>
