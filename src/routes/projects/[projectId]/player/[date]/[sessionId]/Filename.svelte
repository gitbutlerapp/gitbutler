<script lang="ts">
	import { api, stores } from '$lib';
	import type { Bookmark } from '$lib/api';
	import { IconBookmark, IconBookmarkFilled } from '$lib/icons';
	import { collapse } from '$lib/paths';
	import { writable } from 'svelte/store';

	export let projectId: string;
	export let filename: string;
	export let timestampMs: number;

	// TODO: this is stupid, find out why derived stores don't work
	$: bookmarks = stores.bookmarks({ projectId });
	const bookmark = writable<Bookmark | undefined>(undefined);
	$: bookmarks?.subscribe((bookmarks) => {
		if (bookmarks.isLoading) return;
		bookmark.set(bookmarks.value.find((bookmark) => bookmark.timestampMs === timestampMs));
	});
</script>

<div class="flex flex-auto items-center gap-3 overflow-auto">
	<span class="font-mono text-[12px] text-zinc-300">
		{collapse(filename)}
	</span>

	{#if $bookmark}
		<button
			on:click={() =>
				$bookmark &&
				api.bookmarks.upsert(
					$bookmark
						? {
								...$bookmark,
								deleted: !$bookmark.deleted
						  }
						: {
								projectId,
								timestampMs,
								note: '',
								deleted: false
						  }
				)}
		>
			{#if $bookmark.deleted}
				<IconBookmark class="h-4 w-4 text-zinc-700" />
			{:else}
				<IconBookmarkFilled class="h-4 w-4 text-bookmark-selected" />
			{/if}
		</button>
	{/if}
</div>
