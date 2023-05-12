<script lang="ts">
	import type { Project } from '$lib/api';
	import { IconHome } from '$lib/icons';
	import { Tooltip, Button } from '$lib/components';
	import { goto } from '$app/navigation';

	export let project: Project | undefined;
</script>

<div class="flex flex-row items-center gap-1 text-zinc-400">
	<Button icon={IconHome} kind="plain" on:click={() => goto('/')} />
	{#if project}
		<Tooltip label="{project.title} home">
			<Button kind="plain" on:click={() => project && goto(`/projects/${project.id}`)}>
				{project.title}
			</Button>
		</Tooltip>
	{/if}
</div>
