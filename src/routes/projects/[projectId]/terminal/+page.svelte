<script lang="ts">
	import { collapsable } from '$lib/paths';
	import type { LayoutData } from '../$types';
	import Terminal from '$lib/components/Terminal.svelte';
	import * as terminals from '$lib/terminals';
	import Button from '$lib/components/Button/Button.svelte';

	export let data: LayoutData;
	const { project, statuses } = data;

	let terminal: Terminal;
	let terminalSession: terminals.TerminalSession;

	$: if ($project) {
		console.log($project);
		terminalSession = terminals.getTerminalSession($project.id, $project.path);
		console.log('session', terminalSession);
	}

	function runCommand(command: string) {
		terminal.runCommand(command);
	}
</script>

<!-- Actual terminal -->
<div class="terminal-page flex flex h-full w-full flex-row">
	<div class="main-content h-full w-2/3">
		{#if terminalSession}
			<Terminal session={terminalSession} bind:this={terminal} />
		{/if}
	</div>
	<div class="right-panel h-full w-1/3  p-2">
		<h2 class="p-2 pb-4 text-lg font-bold text-zinc-300">Git Status</h2>
		{#if $statuses.length == 0}
			<div class="rounded border border-green-400 bg-green-600 p-2 font-mono text-green-900">
				No changes
			</div>
		{:else}
			<ul class="mx-2 rounded border border-yellow-400 bg-yellow-500 p-2 font-mono text-yellow-900">
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
		<ul class="px-2  ">
			<!-- svelte-ignore a11y-click-events-have-key-events -->
			<Button role="primary" width="full-width" on:click={() => runCommand('git push')}
				>Push Commit</Button
			>
		</ul>
	</div>
</div>
