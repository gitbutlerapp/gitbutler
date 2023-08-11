<script lang="ts">
	import { Link } from '$lib/components';
	import { Branch, BaseBranch, BranchData } from '$lib/vbranches/types';
	import { formatDistanceToNowStrict } from 'date-fns';
	import { IconBranch, IconGitBranch, IconRemote } from '$lib/icons';
	import { IconTriangleDown, IconTriangleUp } from '$lib/icons';
	import { accordion } from './accordion';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import { getContext } from 'svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import Tooltip from '$lib/components/Tooltip/Tooltip.svelte';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import IconHelp from '$lib/icons/IconHelp.svelte';
	import { derived, get, type Loadable, type Readable } from '@square/svelte-store';
	import PeekTray from './PeekTray.svelte';
	import IconRefresh from '$lib/icons/IconRefresh.svelte';
	import IconGithub from '$lib/icons/IconGithub.svelte';
	import TimeAgo from '$lib/components/TimeAgo/TimeAgo.svelte';

	export let vbranchStore: Loadable<Branch[] | undefined>;
	export let remoteBranchStore: Loadable<BranchData[] | undefined>;
	export let baseBranchStore: Readable<BaseBranch | undefined>;
	export let branchController: BranchController;

	$: branchesState = vbranchStore?.state;
	$: remoteBranchesState = remoteBranchStore?.state;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	let yourBranchesOpen = true;
	let remoteBranchesOpen = true;

	let vbViewport: HTMLElement;
	let vbContents: HTMLElement;
	let rbViewport: HTMLElement;
	let rbContents: HTMLElement;
	let rbSection: HTMLElement;
	let baseContents: HTMLElement;

	let selectedItem: Readable<Branch | BranchData | BaseBranch | undefined> | undefined;
	let overlayOffsetTop = 0;
	let peekTrayExpanded = false;
	let fetching = false;

	// TODO: Replace this hacky thing when adding ability to resize sections
	$: yourBranchesMinHeight = Math.min(Math.max($vbranchStore?.length ?? 0, 1), 5) * 3.25;

	function select(detail: Branch | BranchData | BaseBranch | undefined, i: number): void {
		if (peekTrayExpanded && selectedItem && detail == get(selectedItem)) {
			peekTrayExpanded = false;
			return;
		}
		if (detail instanceof Branch) {
			selectedItem = derived(vbranchStore, (branches) =>
				branches?.find((branch) => branch.id == detail.id)
			);
			const element = vbContents.children[i] as HTMLDivElement;
			overlayOffsetTop = element.offsetTop + vbViewport.offsetTop - vbViewport.scrollTop;
		} else if (detail instanceof BranchData) {
			selectedItem = derived(remoteBranchStore, (branches) =>
				branches?.find((remoteBranch) => remoteBranch.sha == detail.sha)
			);
			const element = rbContents.children[i] as HTMLDivElement;
			overlayOffsetTop = element.offsetTop + rbSection.offsetTop - rbViewport.scrollTop;
		} else if (detail instanceof BaseBranch) {
			selectedItem = baseBranchStore;
			overlayOffsetTop = baseContents.offsetTop;
		} else if (detail == undefined) {
			selectedItem = undefined;
		}

		// Skip animation frame so vertical movement happens before transition
		// property is set to include `top`. This way, the box moves smoothly
		// up and down while expanded, but doesn't come flying in at an angle
		// when expanding.
		requestAnimationFrame(() => (peekTrayExpanded = true));
	}

	function onScroll() {
		peekTrayExpanded = false;
	}

	function sumBranchLinesAddedRemoved(branch: Branch) {
		const comitted = branch.commits
			.flatMap((c) => c.files)
			.flatMap((f) => f.hunks)
			.map((h) => h.diff.split('\n'))
			.reduce(
				(acc, lines) => ({
					added: acc.added + lines.filter((l) => l.startsWith('+')).length,
					removed: acc.removed + lines.filter((l) => l.startsWith('-')).length
				}),
				{ added: 0, removed: 0 }
			);
		const uncomitted = branch.files
			.flatMap((f) => f.hunks)
			.map((h) => h.diff.split('\n'))
			.reduce(
				(acc, lines) => ({
					added: acc.added + lines.filter((l) => l.startsWith('+')).length,
					removed: acc.removed + lines.filter((l) => l.startsWith('-')).length
				}),
				{ added: 0, removed: 0 }
			);

		return {
			added: comitted.added + uncomitted.added,
			removed: comitted.removed + uncomitted.removed
		};
	}
</script>

<PeekTray
	base={$baseBranchStore}
	{branchController}
	item={selectedItem}
	offsetTop={overlayOffsetTop}
	fullHeight={true}
	bind:expanded={peekTrayExpanded}
/>
<div
	class="z-30 flex w-80 min-w-[216px] shrink-0 flex-col border-r border-light-400 bg-white text-light-800 dark:border-dark-600 dark:bg-dark-900 dark:text-dark-100"
	style:width={$userSettings.trayWidth ? `${$userSettings.trayWidth}px` : null}
>
	<!-- Base branch -->
	<div
		class="flex flex-col p-2"
		tabindex="0"
		role="button"
		bind:this={baseContents}
		on:click={() => select($baseBranchStore, 0)}
		on:keypress|capture={() => select($baseBranchStore, 0)}
	>
		<div class="flex flex-grow items-center">
			<div class="flex flex-grow items-center gap-1">
				<span class="font-bold">Trunk</span>
				{#if ($baseBranchStore?.behind || 0) > 0}
					<Tooltip label="Unmerged upstream commits">
						<div
							class="flex h-4 w-4 items-center justify-center rounded-full bg-red-500 text-xs font-bold text-white"
						>
							{$baseBranchStore?.behind}
						</div>
					</Tooltip>
				{/if}
			</div>
			<div class="flex">
				<Tooltip label="Fetch from upstream">
					<button
						class="h-5 w-5 items-center justify-center hover:bg-light-150 dark:hover:bg-dark-700"
						on:click|stopPropagation={() => {
							fetching = true;
							branchController.fetchFromTarget().finally(() => (fetching = false));
						}}
					>
						<div class:animate-spin={fetching}>
							<IconRefresh class="h-4 w-4" />
						</div>
					</button>
				</Tooltip>
			</div>
		</div>
		<div class="flex flex-grow items-center text-sm">
			<div class="flex flex-grow items-center gap-1">
				{#if $baseBranchStore?.remoteUrl.includes('github.com')}
					<IconGithub class="h-2.5 w-2.5" />
				{:else}
					<IconBranch class="h-2.5 w-2.5" />
				{/if}
				{$baseBranchStore?.branchName}
			</div>
			<div>
				<Tooltip label="Last fetch from upstream">
					{#if $baseBranchStore?.fetchedAt}
						<TimeAgo date={$baseBranchStore.fetchedAt} />
					{/if}
				</Tooltip>
			</div>
		</div>
	</div>
	<!-- Your branches -->
	<div
		class="flex items-center justify-between border-b border-t border-light-400 bg-light-100 px-2 py-1 pr-1 dark:border-dark-600 dark:bg-dark-800"
	>
		<div class="font-bold">Your Virtual Branches</div>
		<div class="flex h-4 w-4 justify-around">
			<button class="h-full w-full" on:click={() => (yourBranchesOpen = !yourBranchesOpen)}>
				{#if yourBranchesOpen}
					<IconTriangleUp />
				{:else}
					<IconTriangleDown />
				{/if}
			</button>
		</div>
	</div>
	<div use:accordion={yourBranchesOpen} style:min-height={`${yourBranchesMinHeight}rem`}>
		<div
			bind:this={vbViewport}
			on:scroll={onScroll}
			class="hide-native-scrollbar relative flex max-h-full flex-grow flex-col overflow-y-scroll dark:bg-dark-900"
		>
			<div bind:this={vbContents}>
				{#if $branchesState?.isLoading}
					<div class="px-2 py-1">Loading...</div>
				{:else if $branchesState?.isError}
					<div class="px-2 py-1">Something went wrong!</div>
				{:else if !$vbranchStore || $vbranchStore.length == 0}
					<div class="p-4 text-light-700">You currently have no virtual branches.</div>
				{:else}
					{#each $vbranchStore as branch, i (branch.id)}
						{@const { added, removed } = sumBranchLinesAddedRemoved(branch)}
						{@const latestModifiedAt = branch.files.at(0)?.hunks.at(0)?.modifiedAt}
						<div
							role="button"
							tabindex="0"
							on:click={() => select(branch, i)}
							on:keypress|capture={() => select(branch, i)}
							class="border-b border-light-400 p-2 dark:border-dark-600"
							class:bg-light-50={$selectedItem == branch && peekTrayExpanded}
						>
							<div class="flex flex-row items-center">
								<div class="flex-grow truncate text-black dark:text-white">
									{branch.name}
								</div>
								<div class="font-mono text-sm font-bold">
									<span class="text-green-500">
										+{added}
									</span>
									<span class="text-red-500">
										-{removed}
									</span>
								</div>
							</div>
							<div class="flex items-center text-sm text-light-700 dark:text-dark-300">
								{#if latestModifiedAt}
									<div class="flex-grow">
										<TimeAgo date={latestModifiedAt} />
									</div>
								{/if}
								{#if !branch.active}
									<div class="mr-2">
										{#if !branch.baseCurrent}
											<!-- branch will cause merge conflicts if applied -->
											<Tooltip label="Will introduce merge conflicts if applied">
												<div class="text-yellow-500">&#9679;</div>
											</Tooltip>
										{:else if branch.mergeable}
											<Tooltip label="Can be applied cleanly">
												<div class="text-green-500">&#9679;</div>
											</Tooltip>
										{:else}
											<Tooltip
												label="Canflicts with changes in your working directory, cannot be applied"
											>
												<div class="text-red-500">&#9679;</div>
											</Tooltip>
										{/if}
									</div>
								{/if}
							</div>
						</div>
					{/each}
				{/if}
			</div>
		</div>
		<Scrollbar viewport={vbViewport} contents={vbContents} width="0.5rem" />
	</div>

	<!-- Remote branches -->
	<div
		class="flex items-center justify-between border-b border-light-400 bg-light-100 px-2 py-1 pr-1 dark:border-dark-600 dark:bg-dark-800"
	>
		<div class="flex flex-row place-items-center space-x-2">
			<div class="font-bold">Remote Branches</div>
			<a
				target="_blank"
				rel="noreferrer"
				href="https://docs.gitbutler.com/features/virtual-branches/remote-branches"
			>
				<IconHelp class="h-3 w-3 text-light-600" />
			</a>
		</div>
		<div class="flex h-4 w-4 justify-around">
			<button class="h-full w-full" on:click={() => (remoteBranchesOpen = !remoteBranchesOpen)}>
				{#if remoteBranchesOpen}
					<IconTriangleUp />
				{:else}
					<IconTriangleDown />
				{/if}
			</button>
		</div>
	</div>

	<div bind:this={rbSection} use:accordion={remoteBranchesOpen} class="relative">
		<div
			bind:this={rbViewport}
			on:scroll={onScroll}
			class="hide-native-scrollbar flex max-h-full flex-grow flex-col overflow-y-scroll dark:bg-dark-900"
		>
			<div bind:this={rbContents}>
				{#if $remoteBranchesState?.isLoading}
					<div class="px-2 py-1">loading...</div>
				{:else if $remoteBranchesState?.isError}
					<div class="px-2 py-1">Something went wrong</div>
				{:else if !$remoteBranchStore || $remoteBranchStore.length == 0}
					<div class="p-4">
						<p class="mb-2 text-light-700">
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
					{#each $remoteBranchStore as branch, i}
						<div
							role="button"
							tabindex="0"
							on:click|capture={(e) => select(branch, i)}
							on:keypress|capture={(e) => select(branch, i)}
							class:bg-light-50={$selectedItem == branch && peekTrayExpanded}
							class="flex flex-col justify-between gap-1 border-b border-light-400 px-2 py-1 pt-2 dark:border-dark-600"
						>
							<div class="flex flex-row items-center gap-x-2 pr-1">
								<div class="text-light-600 dark:text-dark-200">
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
								<div class="flex-grow truncate text-black dark:text-white" title={branch.name}>
									{branch.name
										.replace('refs/remotes/', '')
										.replace('origin/', '')
										.replace('refs/heads/', '')}
								</div>
							</div>
							<div
								class="flex flex-row justify-between space-x-2 rounded p-1 pr-1 text-light-700 dark:text-dark-300"
							>
								<div class="flex-grow-0 text-sm">
									<TimeAgo date={branch.lastCommitTs} />
								</div>
								<div class="flex flex-grow-0 flex-row space-x-2">
									<Tooltip
										label="This branch has {branch.ahead} commits not on your base branch and your base has {branch.behind} commits not on this branch yet"
									>
										<div class="rounded-lg bg-zinc-100 px-1 text-sm">
											{branch.ahead}/{branch.behind}
										</div>
									</Tooltip>
									{#if !branch.mergeable}
										<div class="font-bold text-red-500" title="Can't be merged">!</div>
									{/if}
								</div>
								<div
									class="isolate flex flex-grow justify-end -space-x-2 overflow-hidden transition duration-300 ease-in-out hover:space-x-1 hover:transition hover:ease-in"
								>
									{#each branch.authors as author}
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
						</div>
					{/each}
				{/if}
			</div>
		</div>
		<Scrollbar viewport={rbViewport} contents={rbContents} width="0.5rem" />
	</div>
</div>
