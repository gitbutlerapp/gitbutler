<script lang="ts">
	import type { PageData } from './$types';
	import { listFiles } from '$lib/sessions';
	import { type Delta, list as listDeltas } from '$lib/deltas';
	import { CodeViewer } from '$lib/components';
	import { IconPlayerPauseFilled, IconPlayerPlayFilled } from '@tabler/icons-svelte';
	import slider from '$lib/slider';
	import { onMount } from 'svelte';

	export let data: PageData;

	const { sessions } = data;

	let currentTimestamp = new Date().getTime();

	$: minVisibleTimestamp = Math.max(
		Math.min(currentTimestamp - 12 * 60 * 60 * 1000, $sessions[0].meta.startTimestampMs),
		$sessions.at(-1)!.meta.startTimestampMs
	);

	let maxVisibleTimestamp = new Date().getTime();
	onMount(() => {
		const inverval = setInterval(() => {
			maxVisibleTimestamp = new Date().getTime();
		}, 1000);
		return () => clearInterval(inverval);
	});

	$: visibleSessions = $sessions.filter(
		(session) =>
			session.meta.startTimestampMs >= minVisibleTimestamp ||
			session.meta.lastTimestampMs >= minVisibleTimestamp
	);
	$: earliestVisibleSession = visibleSessions.at(-1)!;

	let deltasBySessionId: Record<string, Promise<Record<string, Delta[]>>> = {};
	$: visibleSessions
		.filter((s) => deltasBySessionId[s.id] === undefined)
		.forEach((s) => {
			deltasBySessionId[s.id] = listDeltas({
				projectId: data.projectId,
				sessionId: s.id
			});
		});

	let docsBySessionId: Record<string, Promise<Record<string, string>>> = {};
	$: if (docsBySessionId[earliestVisibleSession.id] === undefined) {
		docsBySessionId[earliestVisibleSession.id] = listFiles({
			projectId: data.projectId,
			sessionId: earliestVisibleSession.id
		});
	}

	$: visibleDeltasByFilepath = Promise.all(
		visibleSessions.map((s) => deltasBySessionId[s.id])
	).then((deltasBySessionId) =>
		Object.values(deltasBySessionId).reduce((acc, deltasByFilepath) => {
			Object.entries(deltasByFilepath).forEach(([filepath, deltas]) => {
				deltas = deltas.filter((delta) => delta.timestampMs <= currentTimestamp);
				if (acc[filepath] === undefined) acc[filepath] = [];
				acc[filepath].push(...deltas);
			});
			return acc;
		}, {} as Record<string, Delta[]>)
	);

	let frame: {
		filepath: string;
		doc: string;
		deltas: Delta[];
	} | null = null;
	$: visibleDeltasByFilepath
		.then(
			(visibleDeltasByFilepath) =>
				Object.entries(visibleDeltasByFilepath)
					.filter(([_, deltas]) => deltas.length > 0)
					.map(([filepath, deltas]) => [filepath, deltas.at(-1)!.timestampMs] as [string, number])
					.sort((a, b) => b[1] - a[1])
					.at(0)?.[0] ?? null
		)
		.then(async (visibleFilepath) => {
			if (visibleFilepath !== null) {
				frame = {
					deltas:
						(await visibleDeltasByFilepath.then((deltasByFilepath) =>
							deltasByFilepath[visibleFilepath].sort((a, b) => a.timestampMs - b.timestampMs)
						)) || [],
					doc:
						(await docsBySessionId[earliestVisibleSession.id].then(
							(docsByFilepath) => docsByFilepath[visibleFilepath]
						)) || '',
					filepath: visibleFilepath
				};
			}
		});

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

	$: visibleRanges = visibleSessions
		.map(
			({ meta }) =>
				[
					Math.max(meta.startTimestampMs, minVisibleTimestamp),
					Math.min(meta.lastTimestampMs, maxVisibleTimestamp)
				] as [number, number]
		)
		.sort((a, b) => a[0] - b[0])
		.reduce((timeline, range) => {
			const [from, to] = range;
			const last = timeline.at(-1);
			if (last) timeline.push([last[1], from, false]);
			timeline.push([from, to, true]);
			return timeline;
		}, [] as [number, number, boolean][]);

	const rangeWidth = (range: [number, number]) =>
		(100 * (range[1] - range[0])) / (maxVisibleTimestamp - minVisibleTimestamp) + '%';

	const timestampToOffset = (timestamp: number) =>
		((timestamp - minVisibleTimestamp) / (maxVisibleTimestamp - minVisibleTimestamp)) * 100 + '%';

	const offsetToTimestamp = (offset: number) =>
		offset * (maxVisibleTimestamp - minVisibleTimestamp) + minVisibleTimestamp;

	let timeline: HTMLElement;

	const onSelectTimestamp = (e: MouseEvent) => {
		const { left, width } = timeline.getBoundingClientRect();
		const clickOffset = e.clientX;
		const clickPos = Math.min(Math.max((clickOffset - left) / width, 0), 1) || 0;
		currentTimestamp = offsetToTimestamp(clickPos);
	};
</script>

{#if $sessions.length === 0}
	<div class="flex h-full items-center justify-center">
		<div class="text-center">
			<h2 class="text-xl">I haven't seen any changes yet</h2>
			<p class="text-gray-500">Go code something!</p>
		</div>
	</div>
{:else}
	<div class="flex h-full flex-col gap-2 px-4">
		{#if frame !== null}
			<header>
				<h2 class="text-lg">{frame.filepath}</h2>
			</header>

			<div class="flex-auto overflow-auto">
				<CodeViewer filepath={frame.filepath} doc={frame.doc} deltas={frame.deltas} />
			</div>
		{/if}

		<div id="timeline" class="relative w-full py-4" bind:this={timeline}>
			<div
				id="cursor"
				use:slider
				on:drag={({ detail: v }) => (currentTimestamp = offsetToTimestamp(v))}
				class="absolute flex h-12 w-4 cursor-pointer items-center justify-around transition hover:scale-150"
				style:left="calc({timestampToOffset(currentTimestamp)} - 0.5rem)"
			>
				<div class="h-5 w-0.5 rounded-sm bg-white" />
			</div>

			<div class="flex w-full items-center justify-between">
				<div id="from">
					{new Date(minVisibleTimestamp).toLocaleString()}
				</div>

				<div id="to">
					{new Date(maxVisibleTimestamp).toLocaleString()}
				</div>
			</div>

			<div class="w-full">
				<div id="ranges" class="flex w-full items-center gap-1" on:mousedown={onSelectTimestamp}>
					<div
						class="h-2 rounded-sm"
						style:background-color="inherit"
						style:width={rangeWidth([minVisibleTimestamp, visibleRanges[0][0]])}
					/>
					{#each visibleRanges as [from, to, filled]}
						<div
							class="h-2 rounded-sm"
							style:background-color={filled ? '#D9D9D9' : 'inherit'}
							style:width={rangeWidth([from, to])}
						/>
					{/each}
					<div
						class="h-2 rounded-sm"
						style:background-color="inherit"
						style:width={rangeWidth([
							visibleRanges[visibleRanges.length - 1][1],
							maxVisibleTimestamp
						])}
					/>
				</div>
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
{/if}
