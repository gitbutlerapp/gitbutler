<script lang="ts">
	import type { Project } from '$lib/backend/projects';
	import Spacer from '../settings/Spacer.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	export let projects: Project[];
	let hidden = true;

	export function toggle() {
		hidden = !hidden;
	}

	export function hide() {
		hidden = true;
	}

	function changeProject(projectId: string) {
		goto(location.href.replace($page.params.projectId, projectId));
	}
</script>

{#if !hidden}
	<div
		class="absolute top-full z-30 mt-2 w-full rounded border shadow"
		style:background-color="var(--bg-surface)"
		style:border-color="var(--border-surface)"
	>
		{#each projects as project}
			<button
				class="project-link block w-full px-3 py-2 text-left"
				on:click={() => changeProject(project.id)}
			>
				{project.title}
			</button>
			<Spacer type="card" />
		{/each}
	</div>
{/if}

<style lang="postcss">
	.project-link {
		&:hover,
		&:focus {
			background-color: var(--bg-surface-highlight);
		}
	}
</style>
