<script lang="ts">
	import { collapsable } from '$lib/paths';
	import type { PageData } from '$lib/types';
	import Terminal from '$lib/components/Terminal.svelte';

	export let data: PageData;
	const { user, statuses } = data;

	let terminal: Terminal;

	function runCommand(command: string) {
		terminal.runCommand(command);
	}
</script>

<!-- Actual terminal -->
<div class="flex flex-row w-full h-full">
	<div class="w-80 p-2">
		<div class="p-2 font-bold">Git Status</div>
		{#if $statuses}
			<ul class="rounded border border-yellow-400 bg-yellow-500 p-2 font-mono text-yellow-900">
				{#each $statuses as activity}
					<li class="flex w-full gap-2">
						<span
							class:text-left={activity.staged}
							class:text-right={!activity.staged}
							class="w-[3ch] font-semibold">{activity.status.slice(0, 1).toUpperCase()}</span
						>
						<span class="truncate" use:collapsable={{ value: activity.path, separator: '/' }} />
					</li>
				{/each}
			</ul>
		{/if}
		<div class="mt-4 p-2 font-bold">Commands</div>
		<ul class="px-2">
			<li class="cursor-pointer" on:click={() => runCommand('git push')}>git push</li>
		</ul>
	</div>
	<div>
		<Terminal bind:this={terminal} />
	</div>
</div>
