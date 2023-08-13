<script lang="ts">
	import { slide } from 'svelte/transition';
	import { IconTriangleUp, IconTriangleDown } from '$lib/icons';
	import type { BaseBranch } from '$lib/vbranches/types';
	import type { SettingsStore } from '$lib/userSettings';
	import TimeAgo from '$lib/components/TimeAgo/TimeAgo.svelte';
	import Resizer from '$lib/components/Resizer.svelte';

	export let base: BaseBranch | undefined;
	export let userSettings: SettingsStore;

	let viewport: HTMLElement;

	export function createCommitUrl(id: string): string | undefined {
		if (!base) return undefined;
		return base.commitUrl(id);
	}

	function toggleExpanded() {
		const expanded = $userSettings.bottomPanelExpanded;
		userSettings.update((s) => ({ ...s, bottomPanelExpanded: !expanded }));
	}
</script>

<Resizer
	{viewport}
	direction="vertical"
	reverse={true}
	on:height={(e) => {
		userSettings.update((s) => ({
			...s,
			bottomPanelHeight: e.detail
		}));
	}}
/>
<div class="flex w-full flex-col border-t border-light-400 dark:border-dark-600">
	<div
		class="flex h-5 items-center gap-2 text-light-700 hover:text-light-900 dark:text-dark-200 dark:hover:text-dark-100"
	>
		<div class="flex w-full flex-row space-x-2">
			<div
				class="flex flex-grow flex-row items-center gap-x-2 px-2"
				role="button"
				tabindex="0"
				on:click={toggleExpanded}
				on:keypress={toggleExpanded}
			>
				<div class="text-sm font-bold uppercase">Common base</div>
				{#if base?.behind == 0}
					<div class="text-sm">{base.branchName}</div>
				{/if}
				{#if $userSettings.bottomPanelExpanded}
					<IconTriangleDown />
				{:else}
					<IconTriangleUp />
				{/if}
			</div>
			{#if !$userSettings.bottomPanelExpanded && base}
				<div class="pr-4 font-mono text-xs text-light-600">
					<a
						class="underline hover:text-blue-500"
						target="_blank"
						href={createCommitUrl(base.baseSha)}
					>
						{base.baseSha.substring(0, 8)}
					</a>
				</div>
			{/if}
		</div>
	</div>
	{#if $userSettings.bottomPanelExpanded}
		<div
			bind:this={viewport}
			style:height={`${$userSettings.bottomPanelHeight}px`}
			transition:slide={{ duration: 150 }}
		>
			<h1
				class="border-b border-t border-light-400 px-2 text-sm font-bold text-light-700 dark:border-dark-600"
			>
				Recent commits
			</h1>
			{#if base}
				<div
					class="lane-scroll flex w-full flex-col gap-y-1 overflow-y-auto bg-white dark:bg-dark-1100"
				>
					{#each base.recentCommits as commit}
						<div
							class="flex flex-row items-center gap-x-1 border-b border-light-300 px-2 text-light-700 dark:border-dark-700 dark:text-dark-200"
						>
							<div class="w-24 shrink-0 truncate">
								{#if commit.createdAt}
									<TimeAgo date={commit.createdAt} />
								{/if}
							</div>
							<div class="flex w-32 shrink-0 flex-row items-center gap-x-1 truncate">
								<img
									class="relative inline h-3 w-3 rounded-full ring-1 ring-white dark:ring-black"
									title="Gravatar for {commit.author.email}"
									alt="Gravatar for {commit.author.email}"
									srcset="{commit.author.gravatarUrl} 2x"
									width="100"
									height="100"
									on:error
								/>
								<div>{commit.author.name}</div>
							</div>
							<div class="grow truncate">{commit.description.substring(0, 100)}</div>
							<div class="flex-shrink pr-4 font-mono text-sm text-light-600">
								<a
									href={createCommitUrl(commit.id)}
									rel="noreferrer"
									target="_blank"
									class="hover:text-blue-500 hover:underline"
								>
									{commit.id.substring(0, 8)}
								</a>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	{/if}
</div>
