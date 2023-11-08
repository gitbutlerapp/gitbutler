<script lang="ts">
	import { Branch, BaseBranch, RemoteBranch, type CustomStore } from '$lib/vbranches/types';
	import { IconBranch } from '$lib/icons';
	import { IconTriangleDown } from '$lib/icons';
	import { accordion } from './accordion';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import { getContext } from 'svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import { derived, get, type Loadable, type Readable } from '@square/svelte-store';
	import PeekTray from './PeekTray.svelte';
	import IconRefresh from '$lib/icons/IconRefresh.svelte';
	import IconGithub from '$lib/icons/IconGithub.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import Button from '$lib/components/Button.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import type { User } from '$lib/backend/cloud';
	import IconChevronRightSmall from '$lib/icons/IconChevronRightSmall.svelte';
	import { slide } from 'svelte/transition';
	import { computedAddedRemoved } from '$lib/vbranches/fileStatus';
	import RemoteBranches from './RemoteBranches.svelte';
	import type { GitHubIntegrationContext, PullRequest } from '$lib/github/types';
	import PullRequests from './PullRequests.svelte';
	import IconHome from '$lib/icons/IconHome.svelte';
	import Link from '$lib/components/Link.svelte';
	import IconSettings from '$lib/icons/IconSettings.svelte';
	import UpdateButton from './UpdateButton.svelte';
	import type { Update } from '../updater';
	import IconEmail from '$lib/icons/IconEmail.svelte';
	import * as events from '$lib/utils/events';
	import { page } from '$app/stores';

	export let branchesWithContentStore: CustomStore<Branch[] | undefined>;
	export let remoteBranchStore: CustomStore<RemoteBranch[] | undefined>;
	export let baseBranchStore: CustomStore<BaseBranch | undefined>;
	export let pullRequestsStore: CustomStore<PullRequest[] | undefined>;
	export let branchController: BranchController;
	export let peekTransitionsDisabled = false;
	export let projectId: string;
	export let peekTrayExpanded = false;
	export let githubContext: GitHubIntegrationContext | undefined;
	export let user: User | undefined;
	export let update: Loadable<Update>;

	$: branchesState = branchesWithContentStore?.state;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	let yourBranchesOpen = true;

	let applyConflictedModal: Modal;

	let vbViewport: HTMLElement;
	let vbContents: HTMLElement;
	let baseContents: HTMLElement;

	let selectedItem: Readable<RemoteBranch | undefined> | undefined;
	let overlayOffsetTop = 0;
	let fetching = false;

	function select(
		detail: Branch | RemoteBranch | BaseBranch | PullRequest | undefined,
		i: number,
		offset?: number
	): void {
		if (peekTrayExpanded && selectedItem && detail == get(selectedItem)) {
			peekTrayExpanded = false;
			return;
		}
		if (detail instanceof RemoteBranch) {
			selectedItem = derived(remoteBranchStore, (branches) =>
				branches?.find((remoteBranch) => remoteBranch.sha == detail.sha)
			);
			overlayOffsetTop = offset || overlayOffsetTop;
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
		const comitted = computedAddedRemoved(...branch.commits.flatMap((c) => c.files));
		const uncomitted = computedAddedRemoved(...branch.files);

		return {
			added: comitted.added + uncomitted.added,
			removed: comitted.removed + uncomitted.removed
		};
	}

	function toggleBranch(branch: Branch) {
		if (!branch.baseCurrent) {
			applyConflictedModal.show(branch);
		} else {
			branchController.applyBranch(branch.id);
		}
	}
</script>

<PeekTray
	{branchController}
	item={selectedItem}
	offsetTop={overlayOffsetTop}
	fullHeight={true}
	bind:expanded={peekTrayExpanded}
	disabled={peekTransitionsDisabled}
	{projectId}
/>
<div
	class="bg-color-5 border-color-4 z-30 flex w-80 shrink-0 flex-col border-r"
	style:width={$userSettings.trayWidth ? `${$userSettings.trayWidth}px` : null}
	role="menu"
	on:keydown|stopPropagation
	tabindex="0"
>
	<!-- Top spacer -->
	<div class="flex h-5 flex-shrink-0" data-tauri-drag-region></div>
	<!-- Base branch -->
	<a href="/{projectId}/base" class="flex flex-col p-2" tabindex="0" bind:this={baseContents}>
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
					<IconButton
						class="items-center justify-center align-top hover:bg-light-150 dark:hover:bg-dark-700"
						on:click={(e) => {
							fetching = true;
							branchController.fetchFromTarget().finally(() => (fetching = false));
							e.preventDefault();
						}}
					>
						<div class:animate-spin={fetching}>
							<IconRefresh class="h-5 w-5" />
						</div>
					</IconButton>
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
	</a>
	<!-- Your branches -->
	<div
		class="bg-color-4 border-color-4 flex items-center justify-between border-b border-t px-2 py-1 pr-1"
	>
		<div class="flex flex-row place-items-center space-x-2">
			<button class="h-full w-full" on:click={() => (yourBranchesOpen = !yourBranchesOpen)}>
				<IconTriangleDown class={!yourBranchesOpen ? '-rotate-90' : ''} />
			</button>
			<div class="whitespace-nowrap font-bold">Stashed branches</div>
		</div>
		<div class="flex h-4 w-4 justify-around"></div>
	</div>
	<div
		use:accordion={yourBranchesOpen}
		style:height={`${$userSettings.vbranchExpandableHeight}px`}
		class="relative shrink-0"
	>
		<div
			bind:this={vbViewport}
			on:scroll={onScroll}
			class="hide-native-scrollbar flex h-full max-h-full flex-grow flex-col overflow-y-scroll overscroll-none"
		>
			<div bind:this={vbContents}>
				{#if $branchesState.isLoading}
					<div class="px-2 py-1">Loading...</div>
				{:else if $branchesState.isError}
					<div class="px-2 py-1">Something went wrong!</div>
				{:else if !$branchesWithContentStore || $branchesWithContentStore.length == 0}
					<div class="text-color-2 p-2">You currently have no virtual branches</div>
				{:else if $branchesWithContentStore.filter((b) => !b.active).length == 0}
					<div class="text-color-2 p-2">You have no stashed branches</div>
				{:else}
					{#each $branchesWithContentStore.filter((b) => !b.active) as branch, i (branch.id)}
						{@const { added, removed } = sumBranchLinesAddedRemoved(branch)}
						{@const latestModifiedAt = branch.files.at(0)?.hunks.at(0)?.modifiedAt}
						<a
							href={`/${projectId}/stashed/${branch.id}`}
							transition:slide={{ duration: 250 }}
							class="border-color-4 group block border-b p-2 pr-0 -outline-offset-2 outline-blue-200 last:border-b focus-within:outline-2"
							class:bg-light-50={$page.url.pathname.includes(branch.id)}
							class:dark:bg-zinc-700={$page.url.pathname.includes(branch.id)}
						>
							<div class="relative flex max-w-full flex-row">
								<div class="flex flex-shrink flex-grow flex-col gap-y-2 overflow-hidden">
									<div class="text-color-2 flex-grow truncate">
										{branch.name}
									</div>
									<div class="flex shrink-0 items-baseline gap-x-2 text-sm">
										{#if latestModifiedAt}
											<span class="text-color-4"><TimeAgo date={latestModifiedAt} /></span>
										{/if}
										<div class="flex gap-1 font-mono text-sm font-bold">
											<span class="font-mono text-green-500">
												+{added}
											</span>
											<span class="font-mono text-red-500">
												-{removed}
											</span>
										</div>
										{#await branch.isMergeable then isMergeable}
											{#if !branch.active}
												{#if !branch.baseCurrent}
													<!-- branch will cause merge conflicts if applied -->
													<Tooltip label="Will introduce merge conflicts if applied">
														<span class="text-yellow-500">&#9679;</span>
													</Tooltip>
												{:else if !isMergeable}
													<Tooltip
														label="Canflicts with changes in your working directory, cannot be applied"
													>
														<span class="text-red-500">&#9679;</span>
													</Tooltip>
												{:else if isMergeable && (added > 0 || removed > 0)}
													<Tooltip label="Can be applied cleanly">
														<span class="text-green-500">&#9679;</span>
													</Tooltip>
												{/if}
											{/if}
										{/await}
									</div>
								</div>
								<div
									class="shrink-0 self-center overflow-hidden whitespace-nowrap px-2 opacity-0 transition-opacity group-hover:opacity-100 group-focus:opacity-100"
								>
									<IconButton
										icon={IconChevronRightSmall}
										class="text-color-4 hover:text-color-3 flex items-center gap-x-2 p-0 text-sm font-semibold"
										title="apply branch"
										on:click={() => {
											peekTrayExpanded = false;
											toggleBranch(branch);
										}}
									>
										Apply
									</IconButton>
								</div>
							</div>
						</a>
					{/each}
				{/if}
			</div>
		</div>
		<Scrollbar viewport={vbViewport} contents={vbContents} width="0.5rem" />
	</div>

	<Resizer
		minHeight={200}
		viewport={vbViewport}
		direction="vertical"
		class="z-30"
		on:height={(e) => {
			userSettings.update((s) => ({
				...s,
				vbranchExpandableHeight: e.detail
			}));
		}}
	/>

	<!-- Remote branches -->
	{#if githubContext}
		<PullRequests {pullRequestsStore} {projectId} />
	{:else}
		<RemoteBranches
			on:scroll={onScroll}
			on:selection={(e) => select(e.detail.branch, e.detail.i, e.detail.offset)}
			{remoteBranchStore}
			{peekTrayExpanded}
			{selectedItem}
		></RemoteBranches>
	{/if}
	<!-- Bottom spacer -->
	<div
		class="border-color-4 text-color-3 flex flex-shrink-0 items-center justify-between border-t px-4 py-4"
	>
		<div class="flex items-center">
			<Link href="/" class="p-1">
				<IconHome />
			</Link>
			<Link href="/{projectId}/settings" class="p-1">
				<IconSettings />
			</Link>
			<Tooltip label="Send feedback">
				<button class="p-1" on:click={() => events.emit('openSendIssueModal')}>
					<IconEmail />
				</button>
			</Tooltip>
		</div>
		<Link href="/user/">
			{#if user?.picture}
				<img class="mr-1 inline-block h-5 w-5 rounded-full" src={user.picture} alt="Avatar" />
			{/if}
			{user?.name ?? 'Account'}
		</Link>
	</div>
	<!-- App Updatesr -->
	{#if $update?.enabled && $update?.shouldUpdate}
		<div class="border-color-4 flex-shrink-0 border-t px-4 py-4">
			<UpdateButton {update} />
		</div>
	{/if}
</div>

<Modal width="small" bind:this={applyConflictedModal}>
	<svelte:fragment slot="title">Merge conflicts</svelte:fragment>
	<p>Applying this branch will introduce merge conflicts.</p>
	<svelte:fragment slot="controls" let:item let:close>
		<Button height="small" kind="outlined" on:click={close}>Cancel</Button>
		<Button
			height="small"
			color="purple"
			on:click={() => {
				branchController.applyBranch(item.id);
				close();
			}}
		>
			Update
		</Button>
	</svelte:fragment>
</Modal>
