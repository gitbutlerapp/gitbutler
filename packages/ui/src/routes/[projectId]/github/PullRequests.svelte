<script lang="ts">
	import type { GitHubIntegrationContext } from '$lib/github/types';
	import { listPullRequestsWithCache } from '$lib/github/pullrequest';
	import TimeAgo from '$lib/components/TimeAgo/TimeAgo.svelte';
	import { IconPullRequest, IconDraftPullRequest, IconFilter, IconFilterFilled } from '$lib/icons';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import { accordion } from '../accordion';
	import type { PullRequest } from '$lib/github/types';
	import { createEventDispatcher } from 'svelte';
	import { showMenu } from 'tauri-plugin-context-menu';
	import { projectPullRequestListingFilter, ListPRsFilter } from '$lib/config/config';

	export let githubContext: GitHubIntegrationContext;
	export let projectId: string;
	let prs = listPullRequestsWithCache(githubContext);
	$: pullRequestsState = prs.state;

	let rbViewport: HTMLElement;
	let rbContents: HTMLElement;
	let rbSection: HTMLElement;
	let open = true;

	const dispatch = createEventDispatcher<{
		selection: {
			pr: PullRequest;
			i: number;
			offset: number;
		};
	}>();
	function select(pr: PullRequest, i: number) {
		const element = rbContents.children[i] as HTMLDivElement;
		const offset = element.offsetTop + rbSection.offsetTop - rbViewport.scrollTop;
		dispatch('selection', { pr, i, offset });
	}

	const filterChoice = projectPullRequestListingFilter(projectId);
	function filterPRs(prs: PullRequest[], filter: string): PullRequest[] {
		if (filter === ListPRsFilter.ExcludeBots) return prs.filter((pr) => !pr.author?.is_bot);
		return prs;
	}
</script>

<div class="bg-color-4 border-color-4 flex items-center justify-between border-t px-2 py-1 pr-1">
	<div class="flex flex-row place-items-center space-x-2">
		<!-- <button class="h-full w-full" on:click={() => (open = !open)}>
			<IconTriangleDown class={!open ? '-rotate-90' : ''} />
		</button> -->
		<div class="text-color-2 whitespace-nowrap font-bold">Pull Requests</div>
	</div>
	<div class="flex h-4 w-4 justify-center">
		<button
			on:click={() => {
				showMenu({
					items: [
						{
							label: 'Show All',
							event: () => filterChoice.set(ListPRsFilter.All)
						},
						{
							label: 'Exclude Bots',
							event: () => filterChoice.set(ListPRsFilter.ExcludeBots)
						},
						{
							label: 'Only Yours',
							disabled: true,
							event: () => filterChoice.set(ListPRsFilter.OnlyYours)
						}
					]
				});
			}}
		>
			{#if $filterChoice == ListPRsFilter.All}
				<IconFilter class="h-3.5 w-3.5"></IconFilter>
			{:else}
				<IconFilterFilled class="h-3.5 w-3.5"></IconFilterFilled>
			{/if}
		</button>
	</div>
</div>
<div bind:this={rbSection} use:accordion={open} class="border-color-5 relative flex-grow">
	<div
		bind:this={rbViewport}
		on:scroll
		class="hide-native-scrollbar flex max-h-full flex-grow flex-col overflow-y-scroll overscroll-none"
	>
		<div bind:this={rbContents}>
			{#if $pullRequestsState?.isLoading}
				<span>loading...</span>
			{:else if $pullRequestsState?.isError}
				<span>something went wrong</span>
			{:else}
				{#each filterPRs($prs, $filterChoice) as pr, i}
					<div
						role="button"
						tabindex="0"
						on:click={() => select(pr, i)}
						on:keypress={() => select(pr, i)}
						class="border-color-4 flex flex-col justify-between gap-1 border-b px-2 py-1 pt-2 -outline-offset-2 outline-blue-200 last:border-b-0 focus:outline-2"
					>
						<div class="flex flex-row items-center gap-x-2">
							<div>
								{#if pr.draft}
									<IconDraftPullRequest class="text-color-3 h-3.5 w-3.5"></IconDraftPullRequest>
								{:else}
									<IconPullRequest class="h-3.5 w-3.5 text-green-500"></IconPullRequest>
								{/if}
							</div>
							<div class="text-color-2 flex-grow truncate font-semibold" title={pr.title}>
								{pr.title}
							</div>
						</div>
						<div
							class="text-color-4 flex flex-row gap-x-1 whitespace-nowrap text-sm first-letter:items-center"
						>
							<span>
								#{pr.number}
							</span>
							<span>
								opened
								<TimeAgo date={new Date(pr.created_at)} />
							</span>
							by
							<span class="text-color-3 font-semibold">
								{pr.author?.username}
							</span>
							{#if pr.draft}
								(draft)
							{/if}
							{#if pr.author?.is_bot}
								<div
									class="text-color-3 border-color-3 rounded-full border px-1.5 text-xs font-semibold"
								>
									bot
								</div>
							{/if}
						</div>
					</div>
				{/each}
			{/if}
		</div>
	</div>
	<Scrollbar viewport={rbViewport} contents={rbContents} width="0.5rem" />
</div>
