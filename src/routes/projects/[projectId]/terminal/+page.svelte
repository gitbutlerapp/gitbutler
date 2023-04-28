<script lang="ts">
	import type { LayoutData } from '../$types';
	import ResizeObserver from 'svelte-resize-observer';
	import setupTerminal from './terminal';
	import 'xterm/css/xterm.css';
	import type { Project } from '$lib/api';
	import { debounce } from '$lib/utils';
	import { Button, Statuses } from '$lib/components';

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

<div class="terminal-page flex h-full w-full flex-auto flex-row">
	<div class="side-panel h-full w-[424px]  flex-auto p-4">
		<h2 class="pb-4 text-lg font-bold text-zinc-300">Git Status</h2>
		<Statuses statuses={$statuses} />
		<div class="mt-4 font-bold">Commands</div>
		<ul class="py-2">
			<Button role="primary" width="full-width" on:click={() => runCommand('git push')}>
				Push Commit
			</Button>
		</ul>
	</div>

	<div class="main-content h-full w-2/3 flex-auto">
		<div class="flex h-full w-full flex-row">
			<div class="h-full w-full" use:terminal={{ project: $project }} />
			<ResizeObserver on:resize={handleTerminalResize} />
		</div>
	</div>
</div>
