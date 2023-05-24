<script lang="ts">
	import { api } from '$lib';
	import type { Bookmark } from '$lib/api';
	import { IconBookmark, IconBookmarkFilled } from '$lib/icons';
	import { collapse } from '$lib/paths';
	import type { Loadable } from 'svelte-loadable-store';

	export let projectId: string;
	export let filename: string;
	export let bookmarks: Loadable<Bookmark[]>;
	export let timestampMs: number;

	$: bookmark = bookmarks.isLoading
		? undefined
		: bookmarks.value.find((bookmark) => bookmark.timestampMs === timestampMs);
</script>

<div
	class="ml-2 flex flex max-w-full flex-auto items-center gap-3 gap-2 overflow-auto rounded-full bg-zinc-900/80 py-2 px-4 shadow"
	style="border: 0.5px solid rgba(63, 63, 70, 0.5);
    -webkit-backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
    background-color: rgba(1, 1, 1, 0.6);
"
>
	<span class="font-mono text-[12px] text-zinc-300">
		{collapse(filename)}
	</span>

	<button
		on:click={() =>
			api.bookmarks.upsert(
				bookmark
					? {
							...bookmark,
							deleted: !bookmark.deleted
					  }
					: {
							projectId,
							timestampMs,
							note: '',
							deleted: false
					  }
			)}
	>
		{#if bookmark?.deleted}
			<IconBookmark class="h-4 w-4 text-zinc-700" />
		{:else if !bookmark}
			<IconBookmark class="h-4 w-4 text-zinc-700" />
		{:else}
			<IconBookmarkFilled class="h-4 w-4 text-bookmark-selected" />
		{/if}
	</button>
</div>
