<script lang="ts">
	import type { GitHubIntegrationContext } from '$lib/github/types';
	import { listPullRequests } from '$lib/github/pullrequest';
	import TimeAgo from '$lib/components/TimeAgo/TimeAgo.svelte';
	import { IconPullRequest, IconDraftPullRequest } from '$lib/icons';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import { IconTriangleDown, IconTriangleUp } from '$lib/icons';
	import { accordion } from '../accordion';
	import type { PullRequest } from '$lib/github/types';
	import { createEventDispatcher } from 'svelte';

	export let githubContext: GitHubIntegrationContext;
	let pullRequestsPromise = listPullRequests(githubContext);

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
</script>

<div
	class="bg-color-4 border-color-4 flex items-center justify-between border-b border-t px-2 py-1 pr-1"
>
	<div class="flex flex-row place-items-center space-x-2">
		<div class="text-color-2 font-bold">Pull Requests</div>
	</div>
	<div class="flex h-4 w-4 justify-around">
		<button class="h-full w-full" on:click={() => (open = !open)}>
			{#if open}
				<IconTriangleUp />
			{:else}
				<IconTriangleDown />
			{/if}
		</button>
	</div>
</div>
<div bind:this={rbSection} use:accordion={open} class="border-color-5 relative flex-grow border-b">
	<div
		bind:this={rbViewport}
		on:scroll
		class="hide-native-scrollbar flex max-h-full flex-grow flex-col overflow-y-scroll overscroll-none"
	>
		<div bind:this={rbContents}>
			{#await pullRequestsPromise}
				<span>loading...</span>
			{:then prs}
				{#if prs}
					{#each prs as pr, i}
						<div
							role="button"
							tabindex="0"
							on:click={() => select(pr, i)}
							on:keypress={() => select(pr, i)}
							class="border-color-4 flex flex-col justify-between gap-1 border-b px-2 py-1 pt-2 -outline-offset-2 outline-blue-200 last:border-b focus:outline-2"
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
				{:else}
					<span>something went wrong</span>
				{/if}
			{/await}
		</div>
	</div>
	<Scrollbar viewport={rbViewport} contents={rbContents} width="0.5rem" />
</div>
