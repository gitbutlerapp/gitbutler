<script lang="ts">
	import { collapse } from '$lib/paths';
	import { isStaged, isUnstaged, type Status } from '$lib/api/git/statuses';

	export let statuses: Partial<Record<string, Status>>;
	$: pairs = Object.entries(statuses) as [string, Status][];
</script>

<div>
	{#if Object.keys(statuses).length === 0}
		<div class="flex rounded border border-green-700 bg-green-900 p-2 align-middle text-green-400">
			<div class="icon mr-2 h-5 w-5">
				<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20">
					<path
						fill="#4ADE80"
						fill-rule="evenodd"
						d="M2 10a8 8 0 1 0 16 0 8 8 0 0 0-16 0Zm12.16-1.44a.8.8 0 0 0-1.12-1.12L9.2 11.28 7.36 9.44a.8.8 0 0 0-1.12 1.12l2.4 2.4c.32.32.8.32 1.12 0l4.4-4.4Z"
					/>
				</svg>
			</div>
			Everything is committed
		</div>
	{:else}
		<ul
			class="rounded border border-yellow-400 bg-yellow-500 p-2 font-mono text-[12px] text-yellow-900"
		>
			{#each pairs as [path, status]}
				<li class="flex w-full gap-2">
					<div class="flex w-[3ch] justify-between font-semibold">
						<span>
							{#if isStaged(status)}
								{status.staged.slice(0, 1).toUpperCase()}
							{/if}
						</span>
						<span>
							{#if isUnstaged(status)}
								{status.unstaged.slice(0, 1).toUpperCase()}
							{/if}
						</span>
					</div>
					<span class="truncate">
						{collapse(path)}
					</span>
				</li>
			{/each}
		</ul>
	{/if}
</div>
