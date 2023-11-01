<script lang="ts">
	import * as events from '$lib/events';
	import { collapse } from '$lib/paths';
	import { IconBookmark, IconBookmarkFilled } from '$lib/icons';
	import { format } from 'date-fns';
	import { page } from '$app/stores';
	import * as bookmarks from '$lib/api/bookmarks';
	import { getBookmark } from '$lib/stores/bookmarks';

	export let timestampMs: number;
	export let filename: string;

	$: bookmark = getBookmark({ projectId: $page.params.projectId, timestampMs });
	$: bookmarkState = bookmark.state;

	const toggleBookmark = () => {
		if ($bookmarkState?.isLoading) return;
		if ($bookmarkState?.isError) return;
		bookmarks.upsert(
			!$bookmark
				? {
						projectId: $page.params.projectId,
						timestampMs,
						note: '',
						deleted: false
				  }
				: {
						...$bookmark,
						deleted: !$bookmark.deleted
				  }
		);
	};
</script>

{#if !$bookmarkState?.isLoading && !$bookmarkState?.isError}
	<div
		class="flex max-w-[357px] flex-col gap-2 rounded-[18px] px-4 py-2 shadow"
		style="border: 0.5px solid rgba(63, 63, 70, 0.5);
            -webkit-backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
            background: #1B1B21;
        "
	>
		<header class="flex items-center gap-2 overflow-auto">
			<span class="flex-1 text-[12px] font-semibold text-zinc-300">
				{collapse(filename)}
			</span>
			<button on:click={toggleBookmark} class="z-1">
				{#if $bookmark?.deleted}
					<IconBookmark class="h-4 w-4 text-zinc-700" />
				{:else if !$bookmark}
					<IconBookmark class="h-4 w-4 text-zinc-700" />
				{:else}
					<IconBookmarkFilled class="h-4 w-4 text-bookmark-selected" />
				{/if}
			</button>
		</header>

		{#if $bookmark && $bookmark.note.length && !$bookmark.deleted}
			<div
				role="button"
				tabindex="0"
				class="flex cursor-pointer flex-col gap-2"
				on:click={() => events.emit('openBookmarkModal')}
				on:keydown={() => events.emit('openBookmarkModal')}
			>
				<main class="max-h-[7ch] overflow-auto text-text-subdued">
					{$bookmark.note}
				</main>

				<footer class="text-right text-sm text-text-subdued">
					{format(new Date($bookmark.updatedTimestampMs), 'E d MMM  yyyy')}
				</footer>
			</div>
		{/if}
	</div>
{/if}
