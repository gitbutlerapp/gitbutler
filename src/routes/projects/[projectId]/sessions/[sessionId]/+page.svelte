<script lang="ts">
	import MdKeyboardArrowLeft from 'svelte-icons/md/MdKeyboardArrowLeft.svelte';
	import MdKeyboardArrowRight from 'svelte-icons/md/MdKeyboardArrowRight.svelte';
	import type { PageData } from './$types';
	import { add, format, differenceInSeconds, addSeconds } from 'date-fns';
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import { derived } from 'svelte/store';
	import { Operation } from '$lib/deltas';
	import { Slider } from 'fluent-svelte';
	import { CodeViewer } from '$lib/components';
	import 'fluent-svelte/theme.css';

	export let data: PageData;
	$: session = data.session;
	$: previousSesssion = data.previousSesssion;
	$: nextSession = data.nextSession;
	$: deltas = data.deltas;

	let time = new Date();
	$: start = new Date($session.meta.startTimestampMs);
	$: end = $session.hash ? addSeconds(new Date($session.meta.lastTimestampMs), 10) : time; // For some reason, some deltas are stamped a few seconds after the session end.
	// Also, if the session is current, the end time moves.

	onMount(() => {
		const interval = setInterval(() => {
			time = new Date();
		}, 10000);

		return () => {
			clearInterval(interval);
		};
	});
	$: midpoint = add(start, {
		seconds: differenceInSeconds(end, start) * 0.5
	});
	$: quarter = add(start, {
		seconds: differenceInSeconds(end, start) * 0.25
	});
	$: threequarters = add(start, {
		seconds: differenceInSeconds(end, start) * 0.75
	});
	const timeStampToCol = (deltaTimestamp: Date) => {
		if (deltaTimestamp < start || deltaTimestamp > end) {
			console.error(
				`Delta timestamp out of session range. Delta timestamp: ${deltaTimestamp}, Session start: ${start}, Session end: ${end}`
			);
		}
		// there are 88 columns
		// start is column 17
		const totalDiff = differenceInSeconds(end, start);
		const eventDiff = differenceInSeconds(deltaTimestamp, start);
		const rat = eventDiff / totalDiff;
		const col = Math.floor(rat * 63 + 17);
		return col;
	};

	const colToTimestamp = (col: number) => {
		const totalDiff = differenceInSeconds(end, start);
		const colDiff = col - 17;
		const rat = colDiff / 63;
		const eventDiff = totalDiff * rat;
		const timestamp = addSeconds(start, eventDiff);
		return timestamp;
	};

	$: tickSizeMs = Math.floor((end.getTime() - start.getTime()) / 63); // how many ms each column represents
	let selectedFileIdx = 0;
	let value = 0;

	$: doc = derived([data.deltas], ([allDeltas]) => {
		const filePath = Object.keys(allDeltas)[selectedFileIdx];
		const deltas = allDeltas[filePath];

		let text = data.files[filePath] || '';
		if (!deltas) return text;

		const sliderValueTimestampMs = colToTimestamp(value).getTime() + tickSizeMs; // Include the tick size so that the slider value is always in the future
		// Filter operations based on the current slider value
		const operations = deltas
			.filter(
				(delta) =>
					delta.timestampMs >= start.getTime() && delta.timestampMs <= sliderValueTimestampMs
			)
			.sort((a, b) => a.timestampMs - b.timestampMs)
			.flatMap((delta) => delta.operations);

		operations.forEach((operation) => {
			if (Operation.isInsert(operation)) {
				text =
					text.slice(0, operation.insert[0]) +
					operation.insert[1] +
					text.slice(operation.insert[0]);
			} else if (Operation.isDelete(operation)) {
				text =
					text.slice(0, operation.delete[0]) +
					text.slice(operation.delete[0] + operation.delete[1]);
			}
		});

		return text;
	});

	const formatDate = (date: Date) => {
		return new Intl.DateTimeFormat('default', {
			weekday: 'short',
			day: 'numeric',
			hour: 'numeric',
			minute: 'numeric'
		}).format(date);
	};
</script>

<div class="flex flex-col h-full  text-zinc-400 overflow-hidden">
	<header class="flex items-center justify-between flex-none px-8 py-1.5 border-b border-zinc-700">
		<div class="flex items-center justify-start  w-64">
			<a
				href="/projects/{$page.params.projectId}/sessions/{$previousSesssion?.id}"
				class="-ml-2 w-8 h-8 hover:text-zinc-100 {$previousSesssion
					? ''
					: 'opacity-50 pointer-events-none cursor-not-allowed'}"
			>
				<MdKeyboardArrowLeft />
			</a>
			<div class="flex-grow text-center cursor-default grid grid-cols-7">
				<span class="col-span-3">{formatDate(start)}</span>
				<span>&mdash;</span>
				<span class="col-span-3">{formatDate(end)}</span>
			</div>
			<a
				href="/projects/{$page.params.projectId}/sessions/{$nextSession?.id}"
				class="-mr-2 w-8 h-8 hover:text-zinc-100 {$nextSession
					? ''
					: 'text-zinc-700 pointer-events-none cursor-not-allowed'}"
			>
				<MdKeyboardArrowRight />
			</a>
		</div>
	</header>

	<!-- main part -->
	<div
		class="flex flex-col flex-none max-w-full select-none border-b border-zinc-700 h-full overflow-auto"
	>
		<div class="flex flex-col flex-none max-w-full mb-40">
			<!-- sticky header -->
			<div
				class="overflow-hidden sticky top-0 z-30 bg-zinc-800 flex-none shadow shadow-zinc-700 ring-1 ring-zinc-700 ring-opacity-5 mb-1"
			>
				<div class="grid-cols-11 -mr-px  border-zinc-700  grid">
					<div />
					<div class="col-span-2 flex items-center justify-center py-2">
						<span>{format(start, 'hh:mm')}</span>
					</div>
					<div class="col-span-2 flex items-center justify-center py-2">
						<span>{format(quarter, 'hh:mm')}</span>
					</div>
					<div class="col-span-2 flex items-center justify-center py-2">
						<span>{format(midpoint, 'hh:mm')}</span>
					</div>
					<div class="col-span-2 flex items-center justify-center py-2">
						<span>{format(threequarters, 'hh:mm')}</span>
					</div>
					<div class="col-span-2 flex items-center justify-center py-2">
						<span>{format(end, 'hh:mm')}</span>
					</div>
				</div>
				<!-- needle -->
				<div class="grid grid-cols-11">
					<div class="col-span-2 flex items-center justify-center" />
					<div class="-mx-1 col-span-8 flex items-center justify-center">
						<Slider min={17} max={80} step={1} bind:value>
							<svelte:fragment slot="tooltip" let:value>
								{format(colToTimestamp(value), 'hh:mm')}
							</svelte:fragment>
						</Slider>
					</div>
					<div class="col-span-1 flex items-center justify-center" />
				</div>
			</div>
			<div class="flex flex-auto mb-1">
				<div class="grid flex-auto grid-cols-1 grid-rows-1">
					<!-- file names list -->
					<div
						class="bg-col-start-1 col-end-2 row-start-1 grid divide-y divide-zinc-700/20"
						style="grid-template-rows: repeat({Object.keys($deltas).length}, minmax(2rem, 1fr));"
					>
						<!-- <div class="row-end-1 h-7" /> -->

						{#each Object.keys($deltas) as filePath, i}
							<div class="flex {i == selectedFileIdx ? 'bg-zinc-500/70' : ''}">
								<button
									class="z-20 flex justify-end items-center overflow-hidden sticky left-0 w-1/6 leading-5 
                                    {selectedFileIdx == i
										? 'text-zinc-200 cursor-default'
										: 'text-zinc-400 hover:text-zinc-200 cursor-pointer'}"
									on:click={() => (selectedFileIdx = i)}
									title={filePath}
								>
									{filePath.split('/').pop()}
								</button>
							</div>
						{/each}
					</div>

					<!-- col selection -->
					<div
						class="col-start-1 col-end-2 row-start-1 grid"
						style="grid-template-columns: repeat(88, minmax(0, 1fr));"
					>
						<div class="bg-sky-400/60 " style=" grid-column: {value};" />
					</div>
					<!-- time vertical lines -->
					<div
						class="col-start-1 col-end-2 row-start-1 grid-rows-1 divide-x divide-zinc-700/50 grid grid-cols-11"
					>
						<div class="col-span-2 row-span-full" />
						<div class="col-span-2 row-span-full" />
						<div class="col-span-2 row-span-full" />
						<div class="col-span-2 row-span-full" />
						<div class="col-span-2 row-span-full" />
						<div class="col-span-2 row-span-full" />
					</div>

					<!-- actual entries  -->
					<ol
						class="col-start-1 col-end-2 row-start-1 grid"
						style="
                        grid-template-columns: repeat(88, minmax(0, 1fr));
                        grid-template-rows: repeat({Object.keys($deltas)
							.length}, minmax(0px, 1fr)) auto;"
					>
						{#each Object.entries($deltas) as [filePath, fileDeltas], idx}
							{#each fileDeltas as delta}
								<li
									class="relative flex items-center bg-zinc-300 hover:bg-zinc-100 rounded m-0.5 cursor-pointer"
									style="
                                grid-row: {idx + 1} / span 1;
                                grid-column: {timeStampToCol(
										new Date(delta.timestampMs)
									)} / span 1;"
								>
									<button
										class="z-20 h-full flex flex-col w-full items-center justify-center"
										on:click={() => {
											value = timeStampToCol(new Date(delta.timestampMs));
											selectedFileIdx = idx;
										}}
									/>
								</li>
							{/each}
						{/each}
					</ol>
				</div>
			</div>
			<div class="grid grid-cols-11 mt-6">
				<div class="col-span-2" />
				<div class="col-span-8 p-1 bg-zinc-500/70 rounded select-text">
					{#if $doc}
						<CodeViewer value={$doc} />
					{/if}
				</div>
				<div class="" />
			</div>
		</div>
	</div>
</div>
