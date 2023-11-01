<script lang="ts">
	import type { Session } from '$lib/api/sessions';
	import type { Delta } from '$lib/api/deltas';
	import SessionCard from './SessionCard.svelte';

	export let sessions: (Session & {
		deltas: Partial<Record<string, Delta[]>>;
		files: Partial<Record<string, string>>;
	})[];
	export let currentSessionId: string | undefined;
	export let currentFilepath: string;

	$: visibleDeltas = sessions?.map((s) => s.deltas);
	$: visibleFiles = sessions?.map((s) => s.files);

	$: visibleSessions = sessions?.map((session, i) => ({
		...session,
		visible: Object.keys(visibleDeltas[i]).length > 0
	}));
</script>

<header
	class="card-header bg-color-2 flex flex-row justify-between rounded-t px-3 py-2 leading-[21px]"
>
	<div class="relative flex gap-2">
		<div class="relative bottom-[1px] h-4 w-4 text-sm">🧰</div>
		<div>Working History</div>
		<div class="text-zinc-400">
			{visibleSessions?.filter(({ visible }) => visible).length}
		</div>
	</div>
</header>

<ul class="bg-color-3 mr-1 flex h-full flex-col gap-2 overflow-auto rounded-b pb-2 pl-2 pr-1 pt-2">
	{#each visibleSessions || [] as session, i}
		{@const isCurrent = session.id == currentSessionId}
		{#if session.visible && visibleDeltas?.length > 0 && visibleFiles?.length > 0}
			<SessionCard
				{isCurrent}
				{session}
				deltas={visibleDeltas[i]}
				files={visibleFiles[i]}
				{currentFilepath}
			/>
		{/if}
	{:else}
		<div class="mt-4 text-center text-zinc-300">No activities found</div>
	{/each}
</ul>
