<script lang="ts">
	import type { PageData } from './$types';
	import { listFiles, type Session } from '$lib/sessions';
	import { type Delta, list as listDeltas } from '$lib/deltas';
	import { CodeViewer } from '$lib/components';
	import {
		IconPlayerPauseFilled,
		IconPlayerPlayFilled,
		IconPlayerTrackNextFilled,
		IconPlayerTrackPrevFilled
	} from '@tabler/icons-svelte';

	export let data: PageData;

	const { sessions } = data;

	let currentTimestamp = new Date().getTime();

	let [nextSessionIndex, currentSessionIndex, previousSessionIndex] = [1, 0, -1];

	const within = (timestamp: number, session: Session | undefined) =>
		session &&
		session.meta.startTimestampMs >= timestamp &&
		session.meta.lastTimestampMs <= timestamp;

	$: {
		if (within(currentTimestamp, $sessions.at(currentTimestamp))) {
			// noop
		} else if (within(currentTimestamp, $sessions.at(previousSessionIndex))) {
			previousSessionIndex--;
			currentSessionIndex--;
			nextSessionIndex--;
		} else if (within(currentTimestamp, $sessions.at(nextSessionIndex))) {
			previousSessionIndex++;
			currentSessionIndex++;
			nextSessionIndex++;
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

	let interval: ReturnType<typeof setInterval> | undefined;
	let direction: -1 | 1 = 1;
	let speed = 1;
	let oneSecond = 1000;

	const stop = () => {
		clearInterval(interval);
		interval = undefined;
	};
	const play = () => start({ direction, speed });

	const start = (params: { direction: 1 | -1; speed: number }) => {
		if (interval) stop();
		interval = setInterval(() => {
			currentTimestamp += oneSecond * params.direction;
		}, oneSecond / params.speed);
	};

	const backward = () => {
		direction === -1 ? (speed = speed * 2) : ((direction = -1), (speed = 1));
		start({ direction, speed });
	};

	const forward = () => {
		direction === 1 ? (speed = speed * 2) : ((direction = 1), (speed = 1));
		start({ direction, speed });
	};
</script>

<div class="flex h-full flex-col gap-2 px-12">
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

	<div class="mx-auto flex items-center gap-2">
		<button on:click={backward}><IconPlayerTrackPrevFilled class="h-6 w-6" /></button>
		{#if interval}
			<button on:click={stop}><IconPlayerPauseFilled class="h-6 w-6" /></button>
		{:else}
			<button on:click={play}><IconPlayerPlayFilled class="h-6 w-6" /></button>
		{/if}
		<button on:click={forward}><IconPlayerTrackNextFilled class="h-6 w-6" /></button>
	</div>
</div>
