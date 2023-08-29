<script lang="ts">
	import { open } from '@tauri-apps/api/shell';
	import type { Commit } from '$lib/vbranches/types';
	import TimeAgo from '$lib/components/TimeAgo/TimeAgo.svelte';
	import Tooltip from '$lib/components/Tooltip/Tooltip.svelte';

	export let commit: Commit;
    export let isIntegrated = false;
	export let url: string | undefined = undefined;
</script>

<div
	class="w-full truncate rounded border border-light-400 bg-light-50 p-2 text-left dark:border-dark-600 dark:bg-dark-900"
>
	<div class="mb-1 flex justify-between">
		<div class="truncate">
			{#if url}
				<!-- on:click required when there is a stopPropagation on a parent -->
				<a
					href={url}
					on:click={() => {
						if (url) open(url);
					}}
					target="_blank"
					title="Open in browser"
				>
					{commit.description}
				</a>
			{:else}
				{commit.description}
			{/if}
		</div>
		{#if isIntegrated}
			<div>
				<Tooltip label="This commit is integrated into Trunk and will dissapear once you merge it.">
					<i>integrated</i>
				</Tooltip>
			</div>
		{/if}
	</div>

	<div class="flex space-x-1 text-sm text-light-700">
		<img
			class="relative z-20 inline-block h-4 w-4 rounded-full ring-1 ring-white dark:ring-black"
			title="Gravatar for {commit.author.email}"
			alt="Gravatar for {commit.author.email}"
			srcset="{commit.author.gravatarUrl} 2x"
			width="100"
			height="100"
			on:error
		/>
		<div class="flex-grow truncate">{commit.author.name}</div>
		<div class="truncate">
			<TimeAgo date={commit.createdAt} />
		</div>
	</div>
</div>
