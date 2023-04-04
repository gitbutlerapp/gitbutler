<script lang="ts">
	import { collapsable } from '$lib/paths';
	import type { PageData } from '$lib/types';
	import Terminal from '$lib/components/Terminal.svelte';
	import * as terminals from '$lib/terminals';
	import { onMount } from 'svelte';

	export let data: PageData;
	const { _user, statuses } = data;

	let terminal: Terminal;
	let terminalSession: terminals.TerminalSession;

	onMount(() => {
		terminalSession = {
			projectId: data.project.id,
			controller: null,
			element: null,
			pty: null,
			fit: null
		};
	});

	function runCommand(command: string) {
		terminal.runCommand(command);
	}
</script>

<!-- Actual terminal -->
<div class="flex flex-row w-full h-full">
	<div class="w-80 h-full p-2">
		<div class="p-2 font-bold">Git Status</div>
		{#if $statuses.length == 0}
			<div class="rounded border border-green-400 bg-green-600 p-2 font-mono text-green-900">
				No changes
			</div>
		{:else}
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
			<li class="cursor-pointer" on:click={() => runCommand('git push')}>Push Commit</li>
		</ul>
	</div>
	<div class="h-full w-full">
		{#if terminalSession}
			<Terminal session={terminalSession} bind:this={terminal} />
		{/if}
	</div>
</div>
