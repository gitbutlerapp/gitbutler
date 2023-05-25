<script lang="ts">
	import type { Delta } from '$lib/api';
	import { IconPlayerPauseFilled, IconPlayerPlayFilled } from '$lib/icons';
	import { DiffContext } from '$lib/components';
	import { unsubscribe } from '$lib/utils';
	import { onMount } from 'svelte';
	import { hotkeys } from '$lib';
	import type { Readable } from '@square/svelte-store';
	import { type Loadable, derived, Value } from 'svelte-loadable-store';

	export let value: number;
	export let context: number;
	export let fullContext: boolean;
	export let deltas: Readable<Loadable<[string, Delta][][]>>;

	$: maxDeltaIndex = derived(deltas, (deltas) => deltas.flatMap((d) => d).length - 1);

	let interval: ReturnType<typeof setInterval> | undefined;
	let direction: -1 | 1 = 1;
	let speed = 1;
	let oneSecond = 1000;

	$: isPlaying = !!interval;

	const stop = () => {
		clearInterval(interval);
		interval = undefined;
		speed = 1;
	};

	const play = () => start({ direction, speed });

	const start = (params: { direction: 1 | -1; speed: number }) => {
		if (interval) clearInterval(interval);
		interval = setInterval(() => {
			gotoNextDelta();
		}, oneSecond / params.speed);
	};

	const gotoNextDelta = () => {
		if ($maxDeltaIndex.isLoading) return;
		if (Value.isError($maxDeltaIndex.value)) return;
		if (value < $maxDeltaIndex.value) {
			value += 1;
		} else {
			stop();
		}
	};

	const gotoPrevDelta = () => {
		if (value > 0) {
			value -= 1;
		} else {
			stop();
		}
	};

	const speedUp = () => {
		speed = speed * 2;
		start({ direction, speed });
	};

	onMount(() =>
		unsubscribe(
			hotkeys.on('ArrowRight', () => gotoNextDelta()),
			hotkeys.on('ArrowLeft', () => gotoPrevDelta()),
			hotkeys.on('Space', () => {
				if (isPlaying) {
					stop();
				} else {
					play();
				}
			})
		)
	);
</script>

<div class="playback-controller-ui mx-auto flex w-full items-center justify-between gap-2">
	<div class="left-side flex space-x-8">
		<div class="play-button-button-container">
			{#if interval}
				<button
					class="player-button group fill-zinc-400 duration-300 ease-in-out hover:scale-125"
					on:click={stop}
				>
					<IconPlayerPauseFilled
						class="player-button-play icon-pointer h-6 w-6 fill-zinc-400 group-hover:fill-zinc-100 "
					/>
				</button>
			{:else}
				<button
					class="player-button group fill-zinc-400 duration-300 ease-in-out hover:scale-125"
					on:click={play}
				>
					<IconPlayerPlayFilled
						class="player-button-pause icon-pointer h-6 w-6 fill-zinc-400 group-hover:fill-zinc-100"
					/>
				</button>
			{/if}
		</div>

		<div class="back-forward-button-container ">
			<button
				on:click={gotoPrevDelta}
				class="player-button-back group duration-300 ease-in-out hover:scale-125"
			>
				<svg
					width="20"
					height="20"
					viewBox="0 0 20 20"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
					class="icon-pointer h-6 w-6"
				>
					<path
						fill-rule="evenodd"
						clip-rule="evenodd"
						d="M13.7101 16.32C14.0948 16.7047 14.0955 17.3274 13.7117 17.7111C13.3254 18.0975 12.7053 18.094 12.3206 17.7093L5.37536 10.7641C5.18243 10.5711 5.0867 10.32 5.08703 10.069C5.08802 9.81734 5.18374 9.56621 5.37536 9.37458L12.3206 2.42932C12.7055 2.04445 13.328 2.04396 13.7117 2.42751C14.0981 2.81386 14.0946 3.43408 13.7101 3.81863C13.4234 4.10528 7.80387 9.78949 7.52438 10.069C9.59011 12.1474 11.637 14.2469 13.7101 16.32Z"
						fill="none"
						class="fill-zinc-400 group-hover:fill-zinc-100"
					/>
				</svg>
			</button>

			<button
				on:click={gotoNextDelta}
				class="player-button-forward group duration-300 ease-in-out hover:scale-125"
			>
				<svg
					width="20"
					height="20"
					viewBox="0 0 20 20"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
					class="icon-pointer h-6 w-6"
				>
					<path
						fill-rule="evenodd"
						clip-rule="evenodd"
						d="M6.28991 16.32C5.90521 16.7047 5.90455 17.3274 6.28826 17.7111C6.67461 18.0975 7.29466 18.094 7.67938 17.7093L14.6246 10.7641C14.8176 10.5711 14.9133 10.32 14.913 10.069C14.912 9.81734 14.8163 9.56621 14.6246 9.37458L7.67938 2.42932C7.29451 2.04445 6.67197 2.04396 6.28826 2.42751C5.90192 2.81386 5.90537 3.43408 6.28991 3.81863C6.57656 4.10528 12.1961 9.78949 12.4756 10.069C10.4099 12.1474 8.36301 14.2469 6.28991 16.32Z"
						fill="none"
						class="fill-zinc-400 group-hover:fill-zinc-100"
					/>
				</svg>
			</button>
		</div>

		<button on:click={speedUp}>{speed}x</button>
	</div>

	<DiffContext bind:lines={context} bind:fullContext />
</div>
