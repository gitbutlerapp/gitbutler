<script lang="ts">
	import type { Bookmark, Delta } from '$lib/api';
	import type { Loadable } from 'svelte-loadable-store';
	import slider from './slider';

	type RichSession = { id: string; deltas: [string, Delta][] };
	export let sessions: RichSession[];
	export let value: number;
	export let bookmarks: Loadable<Bookmark[]>;

	$: markers = bookmarks.isLoading
		? ({} as Record<number, string>)
		: (Object.fromEntries(
				bookmarks.value
					.filter(({ deleted }) => !deleted)
					.map(({ timestampMs, note }) => [timestampMs, note])
		  ) as Record<number, string>);

	$: totalDeltas = sessions.reduce((acc, { deltas }) => acc + deltas.length, 0);
	$: chapters = sessions.map((session, index, all) => {
		const from = all.slice(0, index).reduce((acc, { deltas }) => acc + deltas.length, 0);
		const to = from + session.deltas.length;
		const saves = session.deltas.map((delta, deltaIndex) => ({
			value: from + deltaIndex,
			timestampMs: delta[1].timestampMs,
			note: markers[delta[1].timestampMs]
		}));
		return {
			saves,
			from,
			to,
			highlighted: from <= value && value <= to,
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
	<div class="relative flex w-full items-center" bind:this={timeline}>
		{#each chapters as { from, to, width, saves }, i}
			{@const isCurrent = value >= from && value <= to}
			{@const filledPrecentage = Math.max(0, Math.min(100, ((value - from) / (to - from)) * 100))}
			<li
				on:mousedown={(e) => selectValue(e)}
				class="relative flex cursor-pointer items-center"
				style:width="{width}%"
			>
				<div
					class:ml-[3px]={i > 0}
					class:mr-[3px]={i < chapters.length - 1}
					class="h-[6px] w-full rounded-[5px]"
					class:h-[10px]={isCurrent}
					class:rounded-[8px]={isCurrent}
					style:background="linear-gradient(90deg, #2563EB {filledPrecentage}%,
					var(--color-zinc-700) {filledPrecentage}%)"
				/>
			</li>

			{#each saves as save}
				{#if save.note !== undefined}
					<button
						on:click={() => (value = save.value)}
						class="z-1 absolute cursor-pointer rounded-[16px] transition hover:h-[8px] hover:w-[8px] hover:scale-150"
						style:left="calc({valueToOffset(save.value)}% - 4px)"
						class:h-[4px]={!isCurrent}
						class:w-[4px]={!isCurrent}
						class:h-[8px]={isCurrent}
						class:w-[8px]={isCurrent}
						class:bg-[#D4D4D8]={save.value <= value}
						class:bg-[#2563EB]={save.value > value}
					/>
				{/if}
			{/each}
		{/each}
		<div
			id="cursor"
			use:slider
			on:drag={({ detail: offset }) => (value = offsetToValue(offset))}
			class="absolute flex h-[48px] w-[16px] cursor-pointer items-center justify-around transition hover:scale-150"
			style:left="calc({valueToOffset(value)}% - 8px)"
		>
			<div class="h-[18px] w-[3px] rounded-sm bg-white shadow-md" />
		</div>
	</div>
</div>
