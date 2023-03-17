<script lang="ts">
	import { CodeViewer } from '$lib/components';
	import type { Delta } from '$lib/deltas';
	import { formatDistanceToNow } from 'date-fns';
	import { onMount } from 'svelte';

	export let doc: string;
	export let deltas: Delta[];
	export let filepath: string;
	export let mark: string[];

	const timestamp = deltas[deltas.length - 1].timestampMs;

	let viewer: HTMLDivElement;

	const markElement = (node: Element) => {
		if (node.textContent === null) return;
		node.innerHTML = node.innerHTML.replaceAll(
			new RegExp(mark.join('|'), 'g'),
			(match) => `<span style="background: #AC8F2F;">${match}</span>`
		);
	};

	onMount(() => {
		if (mark.length === 0) return;
		for (const elem of viewer.getElementsByClassName('line-changed')) {
			markElement(elem);
		}
	});
</script>

<div class="flex flex-col gap-2">
	<p class="flex justify-between text-lg">
		<span>{filepath}</span>
		<span>{formatDistanceToNow(timestamp)} ago</span>
	</p>
	<div
		bind:this={viewer}
		class="flex-auto overflow-auto rounded-lg border border-zinc-700 bg-[#2F2F33] text-[#EBDBB2] drop-shadow-lg"
	>
		<CodeViewer {doc} {deltas} {filepath} paddingLines={4} />
	</div>
</div>
