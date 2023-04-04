<script lang="ts">
	import type { Delta } from '$lib/deltas';
	import slider from '$lib/slider';
	import { onMount } from 'svelte';

	type RichSession = { id: string; deltas: [string, Delta][] };
	export let sessions: RichSession[];
	export let value: number;

	$: totalDeltas = sessions.reduce((acc, { deltas }) => acc + deltas.length, 0);
	$: sliderValues = sessions.map((session, index, all) => {
		const from = all.slice(0, index).reduce((acc, { deltas }) => acc + deltas.length, 0);
		const to = from + session.deltas.length;
		return {
			from,
			to,
			width: ((to - from) / totalDeltas) * 100
		};
	});

	const valueToOffset = (value: number) => (value / totalDeltas) * 100;

	const offsetToValue = (offset: number) => offset * totalDeltas;

	let timeline: HTMLElement;

	const selectValue = (e: MouseEvent) => {
		const { left, width } = timeline.getBoundingClientRect();
		const clickOffset = e.clientX;
		const clickPos = Math.min(Math.max((clickOffset - left) / width, 0), 1) || 0;
		value = offsetToValue(clickPos);
	};
</script>

<div id="slider">
	<ul
		class="relative flex w-full items-center"
		on:mousedown={(e) => selectValue(e)}
		bind:this={timeline}
	>
		{#each sliderValues as { from, to, width }, i}
			{@const isCurrent = value >= from && value < to}
			{@const filledPrecentage = Math.max(0, Math.min(100, ((value - from) / (to - from)) * 100))}
			<li class="relative flex cursor-pointer items-center" style:width="{width}%">
				<div
					class:ml-[3px]={i > 0}
					class:mr-[3px]={i < sliderValues.length - 1}
					class="h-[6px] w-full bg-zinc-100"
					class:h-[8px]={isCurrent}
					style:background="linear-gradient(90deg, #2563EB {filledPrecentage}%,
					var(--color-zinc-100) {filledPrecentage}%)"
				/>
			</li>
		{/each}
		<div
			id="cursor"
			use:slider
			on:drag={({ detail: offset }) => (value = offsetToValue(offset))}
			class="absolute flex h-[48px] w-[16px] cursor-pointer items-center justify-around transition hover:scale-150"
			style:left="calc({valueToOffset(value)}% - 8px)"
		>
			<div class="h-[18px] w-[2px] rounded-sm bg-white" />
		</div>
	</ul>
</div>
