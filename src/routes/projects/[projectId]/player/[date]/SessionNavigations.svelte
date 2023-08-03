<script lang="ts">
	import { goto } from '$app/navigation';
	import { IconChevronLeft, IconChevronRight } from '$lib/icons';
	import { page } from '$app/stores';
	import { hotkeys } from '$lib';

	import type { Session } from '$lib/api/ipc/sessions';
	import type { Delta } from '$lib/api/ipc/deltas';
	import { unsubscribe } from '$lib/utils';
	import { derived, type Readable } from '@square/svelte-store';
	import { onMount } from 'svelte';
	import { format } from 'date-fns';

	export let sessions: (Session & {
		deltas: Readable<Record<string, Delta[]>>;
	})[];
	export let currentSession: Session;

	$: sessionDeltas = derived(
		sessions.map(({ deltas }) => deltas),
		(deltas) => deltas
	);

	$: nextSessionId = derived(sessionDeltas, (sessionDeltas) => {
		if (sessions) {
			const currentIndex = sessions.findIndex((s) => s.id === currentSession.id);
			if (currentIndex === -1) return undefined;
			for (let i = currentIndex + 1; i < sessions.length; i++) {
				if (Object.keys(sessionDeltas[i]).length > 0) return sessions[i].id;
			}
			return undefined;
		}
	});

	$: prevSessionId = derived(sessionDeltas, (sessionDeltas) => {
		if (sessions) {
			const currentIndex = sessions.findIndex((s) => s.id === currentSession.id);
			if (currentIndex === -1) return undefined;
			for (let i = currentIndex - 1; i >= 0; i--) {
				if (Object.keys(sessionDeltas[i]).length > 0) return sessions[i].id;
			}
			return undefined;
		}
	});

	const removeFromSearchParams = (params: URLSearchParams, key: string) => {
		params.delete(key);
		return params;
	};

	const getSessionURI = (sessionId: string) =>
		`/projects/${$page.params.projectId}/player/${
			$page.params.date
		}/${sessionId}?${removeFromSearchParams($page.url.searchParams, 'delta').toString()}`;

	onMount(() =>
		unsubscribe(
			hotkeys.on('Shift+ArrowRight', () => {
				if ($nextSessionId) goto(getSessionURI($nextSessionId));
			}),
			hotkeys.on('Shift+ArrowLeft', () => {
				if ($prevSessionId) goto(getSessionURI($prevSessionId));
			})
		)
	);
</script>

<span class="min-w-[200px]">
	{format(currentSession.meta.startTimestampMs, 'EEEE, LLL d, HH:mm')}
	-
	{format(currentSession.meta.lastTimestampMs, 'HH:mm')}
</span>

<div class="flex items-center gap-1">
	{#if $prevSessionId && $nextSessionId}
		<a
			href={$prevSessionId && getSessionURI($prevSessionId)}
			class="rounded border border-zinc-500 bg-zinc-600 p-0.5"
			class:hover:bg-zinc-500={!!$prevSessionId}
			class:pointer-events-none={!$prevSessionId}
			class:text-zinc-500={!$prevSessionId}
		>
			<IconChevronLeft class="h-4 w-4" />
		</a>
		<a
			href={$nextSessionId && getSessionURI($nextSessionId)}
			class="rounded border border-zinc-500 bg-zinc-600 p-0.5"
			class:hover:bg-zinc-500={!!$nextSessionId}
			class:pointer-events-none={!$nextSessionId}
			class:text-zinc-500={!$nextSessionId}
		>
			<IconChevronRight class="h-4 w-4" />
		</a>
	{/if}
</div>
