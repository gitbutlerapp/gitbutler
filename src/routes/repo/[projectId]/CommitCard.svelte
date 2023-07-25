<script lang="ts">
	import { formatDistanceToNow } from 'date-fns';
	import type { Commit } from '$lib/vbranches';

	export let commit: Commit;
	export let url: string | undefined = undefined;
</script>

<div class="rounded border border-light-400 bg-light-50 p-2 dark:border-dark-600 dark:bg-dark-900">
	<div class="mb-1 truncate">
		{#if url}
			<a href={url} target="_blank" title="Open in browser">
				{commit.description}
			</a>
		{:else}
			{commit.description}
		{/if}
	</div>
	<div class="flex space-x-1 text-sm text-light-700">
		<img
			class="relative z-30 inline-block h-4 w-4 rounded-full ring-1 ring-white dark:ring-black"
			title="Gravatar for {commit.author.email}"
			alt="Gravatar for {commit.author.email}"
			srcset="{commit.author.gravatarUrl} 2x"
			width="100"
			height="100"
			on:error
		/>
		<div class="flex-grow truncate">{commit.author.name}</div>
		<div class="truncate">{formatDistanceToNow(commit.createdAt)} ago</div>
	</div>
</div>
