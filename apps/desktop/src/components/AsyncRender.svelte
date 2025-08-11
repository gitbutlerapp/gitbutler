<!--
@component
This queueing code makes sure that only one instance of this
component loads per animation frame. The result is a more
progressiver render of the app, rather than waiting a few
hundred milliseconds for e.g. the workspace to appear.
-->
<script module lang="ts">
	let queue: (() => void)[] = [];
	let busy = false;

	function enqueue(callback: () => void) {
		queue.push(callback);
		if (!busy) processNext();
	}

	function processNext() {
		if (queue.length > 0) {
			busy = true;
			for (const fn of queue.splice(0)) {
				fn();
			}
			requestAnimationFrame(processNext);
		} else {
			busy = false;
		}
	}
</script>

<script lang="ts">
	import { type Snippet } from 'svelte';
	import { onMount } from 'svelte';

	const { children }: { children: Snippet } = $props();
	let mounted = $state(false);
	onMount(() => {
		enqueue(() => {
			mounted = true;
		});
	});
</script>

{#if mounted}
	{@render children()}
{/if}
