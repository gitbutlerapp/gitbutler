<script lang="ts">
	import type { Project } from '$lib/api';
	import { IconHome } from '$lib/icons';
	import { Button } from '$lib/components';
	import { goto } from '$app/navigation';

	export let project: Project | undefined;
</script>

<div class="flex flex-row items-stretch gap-1 font-bold text-dark-300 dark:text-light-300">
	<Button icon={IconHome} kind="plain" on:click={() => goto('/')} />
	{#if project}
		<a
			class="button gb-bg-2 flex items-center px-2"
			class:active={window.location.pathname.includes(`/projects/${project.id}`)}
			href="/projects/{project.id}"
			title="{project.title} home"
		>
			{project.title}
		</a>
	{/if}
</div>

<style lang="postcss">
</style>
