<script lang="ts">
	import { collapsable } from '$lib/paths';
	import type { LayoutData } from '../$types';
	import ResizeObserver from 'svelte-resize-observer';
	import setupTerminal from './terminal';
	import { Status } from '$lib/git/statuses';
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

	$: terminal = (target: HTMLElement, params: { project: Project }) => {
		let setupPromise = setupTerminal(params);
		setupPromise.then((terminal) => (term = terminal));
		setupPromise.then((terminal) => terminal.bind(target));
		return {
			update: (params: { project: Project }) => {
				setupPromise.then((term) => term.destroy());
				setupPromise = setupTerminal(params);
				setupPromise.then((terminal) => (term = terminal));
				setupPromise.then((terminal) => terminal.bind(target));
			}
		};
	};
</script>

<div class="terminal-page flex h-full w-full flex-row">
	<div class="side-panel h-full w-1/3  p-4">
		<h2 class="pb-4 text-lg font-bold text-zinc-300">Git Status</h2>
		{#if Object.keys($statuses).length == 0}
			<div class="rounded border border-green-400 bg-green-600 p-2 font-mono text-green-900">
				No changes
			</div>
		{:else}
			<ul
				class="rounded border border-yellow-400 bg-yellow-500 p-2 font-mono text-sm text-yellow-900"
			>
				{#each Object.entries($statuses) as [path, status]}
					<li class="flex w-full gap-2">
						<div class="flex w-[3ch] justify-between font-semibold">
							<span>
								{#if Status.isStaged(status)}
									{status.staged.slice(0, 1).toUpperCase()}
								{/if}
							</span>
							<span>
								{#if Status.isUnstaged(status)}
									{status.unstaged.slice(0, 1).toUpperCase()}
								{/if}
							</span>
						</div>
						<span class="truncate" use:collapsable={{ value: path, separator: '/' }} />
					</li>
				{/each}
			</ul>
		{/if}
		<div class="mt-4 font-bold">Commands</div>
		<ul class="py-2">
			<Button role="primary" width="full-width" on:click={() => runCommand('git push')}>
				Push Commit
			</Button>
		</ul>
	</div>

	<div class="main-content h-full w-2/3">
		<div class="flex h-full w-full flex-row">
			<div class="h-full w-full" use:terminal={{ project: $project }} />
			<ResizeObserver on:resize={handleTerminalResize} />
		</div>
	</div>
</div>
