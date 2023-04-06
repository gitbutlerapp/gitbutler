<script lang="ts">
	import { collapsable } from '$lib/paths';
	import type { LayoutData } from '../$types';
	import ResizeObserver from 'svelte-resize-observer';
	import setupTerminal from './terminal';
	import 'xterm/css/xterm.css';
	import type { Project } from '$lib/projects';
	import { debounce } from '$lib/utils';
	import { Button } from '$lib/components';

	export let data: LayoutData;
	const { project, statuses } = data;

	type Unpromisify<T> = T extends Promise<infer U> ? U : T;
	let term: Unpromisify<ReturnType<typeof setupTerminal>> | undefined;

	const handleTerminalResize = debounce(() => term?.resize(), 5);
	const runCommand = (command: string) => term?.run(command);

	const terminal = (target: HTMLElement, params: { project: Project }) => {
		let setupPromise = setupTerminal(target, params);
		setupPromise.then((terminal) => (term = terminal));
		return {
			update: (params: { project: Project }) => {
				setupPromise.then((term) => term.destroy());
				setupPromise = setupTerminal(target, params);
				setupPromise.then((terminal) => (term = terminal));
			},
			destroy: () => setupPromise.then((term) => term.destroy())
		};
	};
</script>

<div class="terminal-page flex flex h-full w-full flex-row">
	<div class="main-content h-full w-2/3">
		<div class="flex h-full w-full flex-row">
			<div class="h-full w-full" use:terminal={{ project: $project }} />
			<ResizeObserver on:resize={handleTerminalResize} />
		</div>
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
			<Button role="primary" width="full-width" on:click={() => runCommand('git push')}>
				Push Commit
			</Button>
		</ul>
	</div>
</div>
