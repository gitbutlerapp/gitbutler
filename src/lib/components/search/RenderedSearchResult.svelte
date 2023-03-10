<script lang="ts">
	import { formatDistanceToNow } from 'date-fns';
	import type { ProcessedSearchResult } from '.';

	export let processedResult: ProcessedSearchResult;
</script>

<div class="flex flex-col">
	<div class="mb-4">
		<p class="mb-2 flex text-lg text-zinc-400">
			<span>{processedResult.searchResult.filePath}</span>
			<span class="flex-grow" />
			<span>{formatDistanceToNow(processedResult.timestamp)} ago</span>
		</p>
		<div class="rounded-lg text-[#EBDBB2] bg-[#2F2F33] border border-zinc-700 drop-shadow-lg">
			{#each processedResult.hunks as hunk, i}
				{#if i > 0}
					<div class="border-b border-[#52525B]" />
				{/if}
				<div class="flex flex-col px-6 py-3">
					{#each hunk.lines as line}
						{#if !line.hidden}
							<div class="mb-px flex font-mono leading-4">
								<span class="w-6 flex-shrink text-[#928374]"
									>{line.lineNumber ? line.lineNumber : ''}</span
								>
								<pre
									class="flex-grow rounded-sm 
												{line.operation === 'add'
										? 'bg-[#14FF00]/20'
										: line.operation === 'remove'
										? 'bg-[#FF0000]/20'
										: ''}
												">{line.contentBeforeHit}<span class="rounded-sm bg-[#AC8F2F]">{line.contentAtHit}</span
									>{line.contentAfterHit}</pre>
							</div>
						{:else}
							<!-- <span>hidden</span> -->
						{/if}
					{/each}
				</div>
			{/each}
		</div>
	</div>
</div>
