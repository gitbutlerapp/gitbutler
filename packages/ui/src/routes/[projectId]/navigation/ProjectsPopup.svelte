<script lang="ts">
	import type { Project } from '$lib/backend/projects';
	import Spacer from '../settings/Spacer.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { clickOutside } from './clickOutside';
	import * as events from '$lib/utils/events';

	export let projects: Project[];
	let hidden = true;

	export function show() {
		hidden = false;
	}

	function changeProject(projectId: string) {
		goto(location.href.replace($page.params.projectId, projectId));
	}
</script>

{#if !hidden}
	<div
		class="absolute top-full z-30 mt-2 w-full rounded shadow"
		style:background-color="var(--bg-card-highlight)"
		use:clickOutside={() => {
			hidden = true;
		}}
	>
		{#each projects as project}
			<button class="block px-3 py-2" on:click={() => changeProject(project.id)}>
				{project.title}
			</button>
			<Spacer type="card" />
		{/each}
		<button
			class="px-3 py-2"
			on:click={() => {
				events.emit('openNewProjectModal');
			}}>Add project</button
		>
	</div>
{/if}
