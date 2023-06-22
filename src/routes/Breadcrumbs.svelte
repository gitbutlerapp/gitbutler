<script lang="ts">
	import type { Project } from '$lib/api';
	import { IconHome } from '$lib/icons';
	import { Button } from '$lib/components';
	import { goto } from '$app/navigation';

	export let project: Project | undefined;
</script>

<div class="flex flex-row items-stretch gap-1 font-bold ">
	<Button icon={IconHome} kind="plain" on:click={() => goto('/')} />
	{#if project}
		<a
			class="button flex items-center px-3"
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
