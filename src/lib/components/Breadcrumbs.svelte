<script lang="ts">
	import type { Project } from '$lib/api';
	import { IconHome } from './icons';
	import { Tooltip, Button } from '$lib/components';
	import { goto } from '$app/navigation';

	export let project: Project | undefined;
</script>

<div class="flex flex-row items-center gap-1 text-zinc-400">
	<Button icon={IconHome} filled={false} on:click={() => goto('/')} />
	{#if project}
		<Tooltip label="{project.title} home">
			<Button filled={false} on:click={() => project && goto(`/projects/${project.id}`)}>
				{project.title}
			</Button>
		</Tooltip>
	{/if}
</div>
