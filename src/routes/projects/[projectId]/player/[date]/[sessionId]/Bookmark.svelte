<script lang="ts">
	import { events, stores } from '$lib';
	import { format } from 'date-fns';

	export let projectId: string;
	export let timestampMs: number;

	$: bookmark = stores.bookmarks.get({ projectId, timestampMs });
</script>

{#if !$bookmark.isLoading && $bookmark.value && $bookmark.value.note.length && !$bookmark.value.deleted}
	<button
		class="flex max-h-[109px] max-w-[357px] cursor-pointer flex-col gap-1 rounded-xl bg-zinc-900/80 py-3 px-4 shadow"
		style="border: 0.5px solid rgba(63, 63, 70, 0.5);
            -webkit-backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
            background-color: rgba(1, 1, 1, 0.6);
        "
		on:click={() => events.emit('openBookmarkModal')}
	>
		<main class="text-text-subdued">
			{$bookmark.value.note}
		</main>
		<footer class="text-right text-sm text-text-subdued">
			{format(new Date($bookmark.value.updatedTimestampMs), 'E d MMM  yyyy')}
		</footer>
	</button>
{/if}
