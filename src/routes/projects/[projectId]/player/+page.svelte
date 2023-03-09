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

	$: minVisibleTimestamp = currentTimestamp - 12 * 60 * 60 * 1000;

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

	listDeltas({
		projectId: '0317f7eb-0331-4d27-a73b-15170d91bb42',
		sessionId: 'f6387eb5-2017-479f-b657-a5e873c5442d'
	})
		.then((r) => r['src/routes/projects/[projectId]/player/+page.svelte'].map((d) => d.timestampMs))
		.then((tt) => tt.map((timestamp) => new Date(timestamp)))
		.then((ttt) => {
			const s = $sessions.find((s) => s.id === 'f6387eb5-2017-479f-b657-a5e873c5442d')!;
			console.log({
				ttt,
				from: new Date(s.meta.startTimestampMs),
				to: new Date(s.meta.lastTimestampMs)
			});
		});

	listDeltas({
		projectId: '0317f7eb-0331-4d27-a73b-15170d91bb42',
		sessionId: 'ab3fe2ab-da8b-47d1-af95-81488132608f'
	})
		.then((r) => r['src/routes/projects/[projectId]/player/+page.svelte'].map((d) => d.timestampMs))
		.then((tt) => tt.map((timestamp) => new Date(timestamp)))
		.then((ttt) => {
			const s = $sessions.find((s) => s.id === 'ab3fe2ab-da8b-47d1-af95-81488132608f')!;
			console.log({
				ttt,
				from: new Date(s.meta.startTimestampMs),
				to: new Date(s.meta.lastTimestampMs)
			});
		});

	$: console.log($sessions);

	$: {
		const entries = Object.entries(deltasBySessionId);
		const values = entries.map((entry) => entry[1]);
		const keys = entries.map((entry) => entry[0]);
		Promise.all(values).then((sessionsDeltas) => {
			const seen = {} as Record<string, Record<string, string>>;
			sessionsDeltas.forEach((deltas, i) => {
				const sessionId = keys[i];
				Object.entries(deltas).forEach(([filepath, deltas]) =>
					deltas.forEach((delta) => {
						seen[filepath] ??= {};
						if (seen[filepath][delta.timestampMs] !== undefined) {
							console.log(
								`duplicate delta for ${filepath} in ${
									seen[filepath][delta.timestampMs]
								} and ${sessionId}`
							);
						} else {
							seen[filepath][delta.timestampMs] = sessionId;
						}
					})
				);
			});
			console.log({ seen });
		});
	}

	let deltasByFilepath: Record<string, Delta[]> = {};
	$: Promise.all(Object.values(deltasBySessionId)).then((sessionsDeltas) => {
		deltasByFilepath = sessionsDeltas.reduce((acc, deltas) => {
			Object.entries(deltas).forEach(([filepath, deltas]) => {
				acc[filepath] = [...(acc[filepath] ?? []), ...deltas];
			});
			return acc;
		}, {} as Record<string, Delta[]>);
	});

	console.log({ deltasByFilepath });

	$: currentFilepath =
		Object.entries(deltasByFilepath)
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
		? (deltasByFilepath[currentFilepath] ?? []).filter(
				(delta) => delta.timestampMs <= currentTimestamp
		  )
		: null;

	let currentDoc: string | null = null;
	$: {
		docsBySessionId[earliestVisibleSession.id].then((docs) => {
			if (currentFilepath !== null) {
				currentDoc = docs[currentFilepath];
			}
		});
	}

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

	$: console.log({ currentDoc, currentDeltas: currentDeltas, currentFilepath: currentFilepath });
</script>

<div class="flex h-full flex-col gap-2 px-4">
	<header>
		<h2 class="text-lg">{currentFilepath}</h2>
	</header>

	<div class="flex-auto overflow-auto">
		{#if currentDoc !== null && currentDeltas !== null && currentFilepath !== null}
			<CodeViewer doc={currentDoc} filepath={currentFilepath} deltas={currentDeltas} />
		{/if}
	</div>

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
