<script lang="ts">
	import { stores, api, events } from '$lib';
	import { collapse } from '$lib/paths';
	import { IconBookmark, IconBookmarkFilled } from '$lib/icons';
	import { format } from 'date-fns';

	export let projectId: string;
	export let timestampMs: number;
	export let filename: string;

	$: bookmark = stores.bookmarks.get({ projectId, timestampMs });

	const toggleBookmark = () => {
		if ($bookmark.isLoading) return;
		api.bookmarks.upsert(
			!$bookmark.value
				? {
						projectId,
						timestampMs,
						note: '',
						deleted: false
				  }
				: {
						...$bookmark.value,
						deleted: !$bookmark.value.deleted
				  }
		);
	};
</script>

{#if !$bookmark.isLoading}
	<div
		class="flex max-w-[357px] flex-col gap-2 rounded-[18px] py-2 px-4 shadow"
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
				{#if $bookmark.value?.deleted}
					<IconBookmark class="h-4 w-4 text-zinc-700" />
				{:else if !$bookmark.value}
					<IconBookmark class="h-4 w-4 text-zinc-700" />
				{:else}
					<IconBookmarkFilled class="h-4 w-4 text-bookmark-selected" />
				{/if}
			</button>
		</header>

		{#if $bookmark.value && $bookmark.value.note.length && !$bookmark.value.deleted}
			<div
				class="flex cursor-pointer flex-col gap-2"
				on:click={() => events.emit('openBookmarkModal')}
				on:keydown={() => events.emit('openBookmarkModal')}
			>
				<main class="max-h-[7ch] overflow-auto text-text-subdued">
					{$bookmark.value.note}
				</main>

				<footer class="text-right text-sm text-text-subdued">
					{format(new Date($bookmark.value.updatedTimestampMs), 'E d MMM  yyyy')}
				</footer>
			</div>
		{/if}
	</div>
{/if}
