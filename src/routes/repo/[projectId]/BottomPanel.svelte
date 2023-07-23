<script lang="ts">
	import { slide } from 'svelte/transition';
	import { IconTriangleUp, IconTriangleDown } from '$lib/icons';
	import type { BaseBranch, Commit } from '$lib/vbranches';
	import { formatDistanceToNow } from 'date-fns';

	export let target: BaseBranch;

	let shown = false;

	export function createCommitUrl(commit: Commit): string | undefined {
		if (!target) return undefined;
		return `${target.repoBaseUrl}/commit/${commit.id}`;
	}
</script>

<div class="flex border-t border-light-400 dark:border-dark-600">
	<div class="ml-4 flex w-full flex-col">
		<div
			role="button"
			tabindex="0"
			class="flex h-[20px] items-center gap-2 text-light-700 hover:text-light-900 dark:text-dark-200 dark:hover:text-dark-100"
			on:click={() => (shown = !shown)}
			on:keypress={() => (shown = !shown)}
		>
			{#if shown}
				<IconTriangleDown />
			{:else}
				<IconTriangleUp />
			{/if}
			<div class="flex w-full flex-row justify-between space-x-2">
				<div class="flex flex-row space-x-2">
					<div class="text-sm font-bold uppercase">Common base</div>
					{#if target.behind == 0}
						<div class="text-sm">{target.branchName}</div>
					{/if}
				</div>
				{#if !shown}
					<div class="pr-4 font-mono text-xs text-light-600">
						{target.baseSha.substring(0, 8)}
					</div>
				{/if}
			</div>
		</div>
		{#if shown}
			<div class="h-64 py-2" transition:slide={{ duration: 150 }}>
				<h1 class="mb-2 font-bold">Recent Commits</h1>
				<div class="flex w-full flex-col space-y-1 pr-6">
					{#each target.recentCommits as commit}
						<div class="flex flex-row space-x-1 text-light-700">
							<div class="w-24 truncate">{formatDistanceToNow(commit.createdAt)} ago</div>
							<div class="flex w-32 flex-row space-x-1 truncate">
								<img
									class="relative z-30 inline-block h-4 w-4 rounded-full ring-1 ring-white dark:ring-black"
									title="Gravatar for {commit.author.email}"
									alt="Gravatar for {commit.author.email}"
									srcset="{commit.author.gravatarUrl} 2x"
									width="100"
									height="100"
									on:error
								/>
								<div>{commit.author.name}</div>
							</div>
							<div class="flex-grow truncate">{commit.description.substring(0, 200)}</div>
							<div class="flex-shrink pr-4 font-mono text-light-600">
								<a
									href={createCommitUrl(commit)}
									target="_blank"
									class="hover:text-blue-500 hover:underline"
								>
									{commit.id.substring(0, 8)}
								</a>
							</div>
						</div>
					{/each}
				</div>
			</div>
		{/if}
	</div>
</div>
