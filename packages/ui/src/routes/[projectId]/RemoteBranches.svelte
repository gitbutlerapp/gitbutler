<script lang="ts">
	import Link from '$lib/components/Link.svelte';
	import { IconGitBranch, IconRemote } from '$lib/icons';
	import IconHelp from '$lib/icons/IconHelp.svelte';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import { accordion } from './accordion';
	import type { CustomStore, RemoteBranch } from '$lib/vbranches/types';
	import { page } from '$app/stores';

	export let remoteBranchStore: CustomStore<RemoteBranch[] | undefined>;
	export let projectId: string;

	let rbViewport: HTMLElement;
	let rbContents: HTMLElement;
	let rbSection: HTMLElement;
	$: remoteBranchesState = remoteBranchStore?.state;

	let open = false;
</script>

<div
	class="bg-color-4 border-color-4 flex items-center justify-between border-b border-t px-2 py-1 pr-1"
>
	<div class="flex flex-row place-items-center space-x-2">
		<!-- <button class="h-full w-full" on:click={() => (open = !open)}>
			<IconTriangleDown class={!open ? '-rotate-90' : ''} />
		</button> -->
		<div class="text-color-2 whitespace-nowrap font-bold">Remote Branches</div>
		<a
			target="_blank"
			rel="noreferrer"
			href="https://docs.gitbutler.com/features/virtual-branches/remote-branches"
		>
			<IconHelp class="text-color-3 h-3 w-3" />
		</a>
	</div>
	<div class="flex h-4 w-4 justify-around"></div>
</div>

<div bind:this={rbSection} use:accordion={open} class="border-color-5 relative flex-grow">
	<div
		bind:this={rbViewport}
		on:scroll
		class="hide-native-scrollbar flex max-h-full flex-grow flex-col overflow-y-scroll overscroll-none"
	>
		<div bind:this={rbContents}>
			{#if $remoteBranchesState.isLoading}
				<div class="px-2 py-1">loading...</div>
			{:else if $remoteBranchesState.isError}
				<div class="px-2 py-1">Something went wrong</div>
			{:else if !$remoteBranchStore || $remoteBranchStore.length == 0}
				<div class="p-4">
					<p class="text-color-3 mb-2">
						There are no local or remote Git branches that can be imported as virtual branches
					</p>
					<Link
						target="_blank"
						rel="noreferrer"
						href="https://docs.gitbutler.com/features/virtual-branches/remote-branches"
					>
						Learn more
					</Link>
				</div>
			{:else if $remoteBranchStore}
				{#each $remoteBranchStore as branch}
					<a
						href="/{projectId}/remote/{branch.sha}"
						class:bg-color-4={$page.url.pathname.includes(branch.sha)}
						class="border-color-4 flex flex-col justify-between gap-1 border-b px-2 py-1 pt-2 -outline-offset-2 outline-blue-200 last:border-b-0 focus:outline-2"
					>
						<div class="flex flex-row items-center gap-x-2 pr-1">
							<div class="text-color-3">
								{#if branch.name.match('refs/remotes')}
									<Tooltip
										label="This is a remote branch that you don't have a virtual branch tracking yet"
									>
										<IconRemote class="h-4 w-4" />
									</Tooltip>
								{:else}
									<Tooltip label="This is a local branch that is not a virtual branch yet">
										<IconGitBranch class="h-4 w-4" />
									</Tooltip>
								{/if}
							</div>
							<div class="text-color-2 flex-grow truncate" title={branch.name}>
								{branch.name
									.replace('refs/remotes/', '')
									.replace('origin/', '')
									.replace('refs/heads/', '')}
							</div>
						</div>
						<div class="flex flex-row justify-between space-x-2 rounded p-1 pr-1">
							<div class="text-color-4 flex-grow-0 text-sm">
								<TimeAgo date={branch.lastCommitTs()} />
							</div>
							<div class="flex flex-grow-0 flex-row space-x-2">
								<Tooltip
									label="This branch has {branch.ahead()} commits not on your base branch and your base has {branch.behind} commits not on this branch yet"
								>
									<div class="bg-color-3 text-color-3 rounded-lg px-2 text-sm">
										{branch.ahead()} / {branch.behind}
									</div>
								</Tooltip>
								{#if !branch.isMergeable}
									<div class="font-bold text-red-500" title="Can't be merged">!</div>
								{/if}
							</div>
							<div
								class="isolate flex flex-grow justify-end -space-x-2 overflow-hidden transition duration-300 ease-in-out hover:space-x-1 hover:transition hover:ease-in"
							>
								{#each branch.authors() as author}
									<img
										class="relative z-30 inline-block h-4 w-4 rounded-full ring-1 ring-white dark:ring-black"
										title="Gravatar for {author.email}"
										alt="Gravatar for {author.email}"
										srcset="{author.gravatarUrl} 2x"
										width="100"
										height="100"
										on:error
									/>
								{/each}
							</div>
						</div>
					</a>
				{/each}
			{/if}
		</div>
	</div>
	<Scrollbar viewport={rbViewport} contents={rbContents} width="0.5rem" />
</div>
