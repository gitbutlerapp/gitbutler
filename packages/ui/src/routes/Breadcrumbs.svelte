<script lang="ts">
	import type { Project } from '$lib/api/ipc/projects';
	import { IconHome } from '$lib/icons';
	import { goto } from '$app/navigation';

	export let project: Project | undefined;
</script>

<div class="flex flex-row items-stretch gap-1 font-bold text-light-800 dark:text-dark-100">
	<button class="p-1" on:click={() => goto('/')}><IconHome /></button>
	{#if project}
		<a class="button flex items-center px-3" href="/repo/{project.id}" title="{project.title} home">
			{project.title}
		</a>
	{/if}
</div>

<style lang="postcss">
</style>
