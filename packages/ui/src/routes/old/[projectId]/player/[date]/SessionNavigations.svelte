<script lang="ts">
	import { goto } from '$app/navigation';
	import { IconChevronLeft, IconChevronRight } from '$lib/icons';
	import { page } from '$app/stores';
	import * as hotkeys from '$lib/hotkeys';

	import type { Session } from '$lib/api/sessions';
	import type { Delta } from '$lib/api/deltas';
	import { unsubscribe } from '$lib/utils';
	import { onMount } from 'svelte';
	import { format } from 'date-fns';

	export let sessions: (Session & {
		deltas: Partial<Record<string, Delta[]>>;
	})[];
	export let currentSession: Session;

	let nextSessionId: string | undefined;
	let prevSessionId: string | undefined;

	$: sessionDeltas = sessions.map(({ deltas }) => deltas);

	$: if (sessions && currentSession) {
		const currentIndex = sessions.findIndex((s) => s.id === currentSession.id);
		nextSessionId = undefined;
		for (let i = currentIndex + 1; i < sessions.length; i++) {
			if (Object.keys(sessionDeltas[i]).length > 0) {
				nextSessionId = sessions[i].id;
				break;
			}
		}

		prevSessionId = undefined;
		for (let i = currentIndex - 1; i >= 0; i--) {
			if (Object.keys(sessionDeltas[i]).length > 0) {
				prevSessionId = sessions[i].id;
				break;
			}
		}
	}

	const removeFromSearchParams = (params: URLSearchParams, key: string) => {
		params.delete(key);
		return params;
	};

	const getSessionURI = (sessionId: string) =>
		`/old/${$page.params.projectId}/player/${
			$page.params.date
		}/${sessionId}?${removeFromSearchParams($page.url.searchParams, 'delta').toString()}`;

	onMount(() =>
		unsubscribe(
			hotkeys.on('Shift+ArrowRight', () => {
				if (nextSessionId) goto(getSessionURI(nextSessionId));
			}),
			hotkeys.on('Shift+ArrowLeft', () => {
				if (prevSessionId) goto(getSessionURI(prevSessionId));
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
	<a
		href={prevSessionId && getSessionURI(prevSessionId)}
		class="bg-color-4 rounded border p-0.5"
		class:hover:bg-color-5={!!prevSessionId}
		class:pointer-events-none={!prevSessionId}
		class:text-color-4={!prevSessionId}
	>
		<IconChevronLeft class="h-4 w-4" />
	</a>
	<a
		href={nextSessionId && getSessionURI(nextSessionId)}
		class="bg-color-4 rounded border p-0.5"
		class:hover:bg-color-5={!!nextSessionId}
		class:pointer-events-none={!nextSessionId}
		class:text-color-4={!nextSessionId}
	>
		<IconChevronRight class="h-4 w-4" />
	</a>
</div>
