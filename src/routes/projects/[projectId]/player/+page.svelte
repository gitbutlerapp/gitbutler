<script lang="ts">
	import type { PageData } from './$types';
	import { listFiles, Session } from '$lib/sessions';
	import { type Delta, list as listDeltas } from '$lib/deltas';
	import { CodeViewer } from '$lib/components';
	import { IconPlayerPauseFilled, IconPlayerPlayFilled } from '@tabler/icons-svelte';

	export let data: PageData;

	const { sessions } = data;

	let currentTimestamp = new Date().getTime();

	let currentSessionIndex = 0;

	$: {
		if (Session.within($sessions.at(currentTimestamp), currentTimestamp)) {
			// noop
		} else if (Session.within($sessions.at(currentSessionIndex - 1), currentTimestamp)) {
			currentSessionIndex--;
		} else if (Session.within($sessions.at(currentSessionIndex + 1), currentTimestamp)) {
			currentSessionIndex++;
		} else {
			// noop
		}
	}

	let currentSessionFileByFilepath = {} as Record<string, string>;
	$: {
		listFiles({ projectId: data.projectId, sessionId: $sessions.at(currentSessionIndex)!.id }).then(
			(r) => (currentSessionFileByFilepath = r)
		);
	}

	let currentSessionDeltasByFilepath = {} as Record<string, Delta[]>;
	$: {
		listDeltas({
			projectId: data.projectId,
			sessionId: $sessions.at(currentSessionIndex)!.id
		}).then((r) => (currentSessionDeltasByFilepath = r));
	}

	$: currentFilepath =
		Object.entries(currentSessionDeltasByFilepath)
			.map(
				([filepath, deltas]) =>
					[filepath, deltas.filter((delta) => delta.timestampMs <= currentTimestamp)] as [
						string,
						Delta[]
					]
			)
			.filter(([_, deltas]) => deltas.length > 0)
			.map(([filepath, deltas]) => [filepath, deltas.at(-1)!.timestampMs] as [string, number])
			.sort((a, b) => b[1] - a[1])
			.at(0)?.[0] ?? null;

	$: currentDeltas = currentFilepath
		? (currentSessionDeltasByFilepath[currentFilepath] ?? []).filter(
				(delta) => delta.timestampMs <= currentTimestamp
		  )
		: null;

	$: currentDoc = currentFilepath ? currentSessionFileByFilepath[currentFilepath] ?? '' : null;

	// player
	let interval: ReturnType<typeof setInterval> | undefined;
	let direction: -1 | 1 = 1;
	let speed = 1;
	let oneSecond = 1000;

	const stop = () => {
		clearInterval(interval);
		interval = undefined;
		speed = 1;
	};
	const play = () => start({ direction, speed });

	const start = (params: { direction: 1 | -1; speed: number }) => {
		if (interval) clearInterval(interval);
		interval = setInterval(() => {
			currentTimestamp += oneSecond * params.direction;
		}, oneSecond / params.speed);
	};

	const speedUp = () => {
		speed = speed * 2;
		start({ direction, speed });
	};

	// timeline
	$: sessionRanges = $sessions.map(
		({ meta }) => [meta.startTimestampMs, meta.lastTimestampMs] as [number, number]
	);
	$: currentRange = sessionRanges.at(currentSessionIndex)!;
	$: minVisibleTimestamp = currentRange[0] - 12 * 60 * 60 * 1000;
	$: maxVisibleTimestamp = Math.max(currentTimestamp, sessionRanges.at(0)![1]);
	$: visibleRanges = sessionRanges
		.filter(([from]) => from >= minVisibleTimestamp)
		.filter(([_, to]) => to <= maxVisibleTimestamp)
		.sort((a, b) => a[0] - b[0]);
	$: ranges = visibleRanges.reduce((timeline, range) => {
		const [from, to] = range;
		const last = timeline.at(-1);
		if (last) timeline.push([last[1], from, false]);
		timeline.push([from, to, true]);
		return timeline;
	}, [] as [number, number, boolean][]);

	const rangeWidth = (range: [number, number]) =>
		(100 * (range[1] - range[0])) / (maxVisibleTimestamp - minVisibleTimestamp) + '%';

	const timestampOffset = (timestamp: number) =>
		((timestamp - minVisibleTimestamp) / (maxVisibleTimestamp - minVisibleTimestamp)) * 100 + '%';
</script>

<div class="flex h-full flex-col gap-2 px-4">
	<div>
		<div>current filepath {currentFilepath}</div>
		<div>current deltas.length {currentDeltas?.length}</div>
		<div>current doc.length {currentDoc?.length}</div>
		<div>current timestamp {new Date(currentTimestamp).toTimeString()}</div>
	</div>

	<div class="flex-auto overflow-auto">
		{#key currentSessionIndex}
			{#if currentDoc !== null && currentDeltas !== null && currentFilepath !== null}
				<CodeViewer doc={currentDoc} filepath={currentFilepath} deltas={currentDeltas} />
			{/if}
		{/key}
	</div>

	<div id="timeline" class="flex w-full items-center py-4">
		<div class="flex w-full items-center gap-1">
			<div
				id="cursor"
				class="absolute flex h-12 w-4 cursor-pointer items-center justify-around"
				style:left="calc({timestampOffset(currentTimestamp)} - 1.5rem)"
			>
				<div class="h-5 w-0.5 rounded-sm bg-white" />
			</div>

			<div
				class="h-2 rounded-sm"
				style:background-color="inherit"
				style:width={rangeWidth([minVisibleTimestamp, ranges[0][0]])}
			/>
			{#each ranges as [from, to, filled]}
				<div
					class="h-2 rounded-sm"
					style:background-color={filled ? '#D9D9D9' : 'inherit'}
					style:width={rangeWidth([from, to])}
				/>
			{/each}
			<div
				class="h-2 rounded-sm"
				style:background-color="inherit"
				style:width={rangeWidth([ranges[ranges.length - 1][1], maxVisibleTimestamp])}
			/>
		</div>
	</div>

	<div class="mx-auto flex items-center gap-2">
		{#if interval}
			<button on:click={stop}><IconPlayerPauseFilled class="h-6 w-6" /></button>
		{:else}
			<button on:click={play}><IconPlayerPlayFilled class="h-6 w-6" /></button>
		{/if}
		<button on:click={speedUp}>{speed}x</button>
	</div>
</div>
