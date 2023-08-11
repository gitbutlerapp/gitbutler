<script lang="ts">
	import { Button, Checkbox, Link, Modal } from '$lib/components';
	import type { Branch, BranchData } from '$lib/vbranches/types';
	import { formatDistanceToNowStrict } from 'date-fns';
	import { IconGitBranch, IconRemote } from '$lib/icons';
	import { IconTriangleDown, IconTriangleUp } from '$lib/icons';
	import { accordion } from './accordion';
	import PopupMenu from '$lib/components/PopupMenu/PopupMenu.svelte';
	import PopupMenuItem from '$lib/components/PopupMenu/PopupMenuItem.svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import { getContext } from 'svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import Tooltip from '$lib/components/Tooltip/Tooltip.svelte';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import IconMeatballMenu from '$lib/icons/IconMeatballMenu.svelte';
	import IconHelp from '$lib/icons/IconHelp.svelte';
	import type { LoadState } from '@square/svelte-store';

	export let branches: Branch[] | undefined;
	export let branchesState: LoadState;
	export let remoteBranches: BranchData[] | undefined;
	export let remoteBranchesState: LoadState;
	export let branchController: BranchController;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	let yourBranchesOpen = true;
	let remoteBranchesOpen = true;

	let yourBranchContextMenu: PopupMenu;
	let remoteBranchContextMenu: PopupMenu;
	let applyConflictedModal: Modal;
	let deleteBranchModal: Modal;

	let vbViewport: HTMLElement;
	let vbContents: HTMLElement;
	let rbViewport: HTMLElement;
	let rbContents: HTMLElement;

	// TODO: Replace this hacky thing when adding ability to resize sections
	$: yourBranchesMinHeight = Math.min(Math.max(branches?.length ?? 0, 1), 5) * 3.25;

	function toggleBranch(branch: Branch) {
		if (branch.active) {
			branchController.unapplyBranch(branch.id);
		} else if (!branch.baseCurrent) {
			applyConflictedModal.show(branch);
		} else {
			branchController.applyBranch(branch.id);
		}
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

<div
	class="flex w-80 min-w-[216px] shrink-0 flex-col border-r border-light-400 bg-white text-light-800 dark:border-dark-600 dark:bg-dark-900 dark:text-dark-100"
	style:width={$userSettings.trayWidth ? `${$userSettings.trayWidth}px` : null}
>
	<!-- Your branches -->
	<div
		class="flex items-center justify-between border-b border-light-400 bg-light-100 px-2 py-1 pr-1 dark:border-dark-600 dark:bg-dark-800"
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
	<div
		class="relative"
		use:accordion={yourBranchesOpen}
		style:min-height={`${yourBranchesMinHeight}rem`}
	>
		<div
			bind:this={vbViewport}
			class="hide-native-scrollbar relative flex max-h-full flex-grow flex-col overflow-y-scroll dark:bg-dark-900"
		>
			<div bind:this={vbContents}>
				{#if branchesState.isLoading}
					<div class="px-2 py-1">Loading...</div>
				{:else if branchesState.isError}
					<div class="px-2 py-1">Something went wrong!</div>
				{:else if !branches || branches.length == 0}
					<div class="p-4 text-light-700">You currently have no virtual branches.</div>
				{:else}
					{#each branches as branch (branch.id)}
						{@const { added, removed } = sumBranchLinesAddedRemoved(branch)}
						{@const latestModifiedAt = branch.files.at(0)?.hunks.at(0)?.modifiedAt}
						<div
							role="listitem"
							on:contextmenu|preventDefault={(e) => yourBranchContextMenu.openByMouse(e, branch)}
							class="border-b border-light-400 p-2 dark:border-dark-600"
						>
							<div class="flex flex-row items-center">
								<Checkbox
									on:change={() => toggleBranch(branch)}
									bind:checked={branch.active}
									disabled={!(branch.mergeable || !branch.baseCurrent) || branch.conflicted}
								/>
								<div class="ml-2 flex-grow truncate text-black dark:text-white">
									{branch.name}
								</div>
								<button
									class="flex-grow-0 text-light-600 transition-colors dark:text-dark-200"
									on:click={(e) => yourBranchContextMenu.openByMouse(e, branch)}
								>
									<IconMeatballMenu />
								</button>
							</div>
							<div class="flex items-center text-sm text-light-700 dark:text-dark-300">
								<div class="flex-grow">
									{latestModifiedAt ? formatDistanceToNowStrict(latestModifiedAt) + 'ago' : ''}
								</div>
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
								<div class="flex gap-1 font-mono text-sm font-bold">
									<span class="text-green-500">
										+{added}
									</span>
									<span class="text-red-500">
										-{removed}
									</span>
								</div>
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

	<div class="relative" use:accordion={remoteBranchesOpen}>
		<div
			bind:this={rbViewport}
			class="hide-native-scrollbar relative flex max-h-full flex-grow flex-col overflow-y-scroll dark:bg-dark-900"
		>
			<div bind:this={rbContents}>
				{#if remoteBranchesState.isLoading}
					<div class="px-2 py-1">loading...</div>
				{:else if remoteBranchesState.isError}
					<div class="px-2 py-1">Something went wrong</div>
				{:else if !remoteBranches || remoteBranches.length == 0}
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
				{:else if remoteBranches}
					{#each remoteBranches as branch}
						<div
							role="listitem"
							on:contextmenu|preventDefault={(e) => remoteBranchContextMenu.openByMouse(e, branch)}
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
								<button
									class="h-8 w-8 flex-grow-0 p-2 text-light-600 transition-colors hover:bg-zinc-300 dark:text-dark-200 dark:hover:bg-zinc-800"
									on:click={(e) => remoteBranchContextMenu.openByMouse(e, branch)}
								>
									<IconMeatballMenu />
								</button>
							</div>
							<div
								class="flex flex-row justify-between space-x-2 rounded bg-light-100 p-1 pr-1 text-light-700 dark:bg-dark-700 dark:text-dark-300"
							>
								<div class="flex-grow-0 text-sm">
									{formatDistanceToNowStrict(branch.lastCommitTs * 1000)} ago
								</div>
								<div class="flex flex-grow-0 flex-row space-x-2">
									<Tooltip
										label="This branch has {branch.ahead} commits not on your base branch and your base has {branch.behind} commits not on this branch yet"
									>
										<div class="rounded-lg bg-zinc-200 p-1 text-sm">
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

	<!-- Your branches context menu -->
	<PopupMenu bind:this={yourBranchContextMenu} let:item>
		{@const disabled = branches?.some((b) => b.id == item.id && b.active)}
		<PopupMenuItem
			{disabled}
			title={disabled ? 'Unapply before delete' : 'Delete branch'}
			on:click={() => item && deleteBranchModal.show(item)}
		>
			Delete
		</PopupMenuItem>
	</PopupMenu>

	<!-- Remote branches context menu -->
	<PopupMenu bind:this={remoteBranchContextMenu} let:item>
		<PopupMenuItem on:click={() => item && branchController.createvBranchFromBranch(item.name)}
			>Apply</PopupMenuItem
		>
	</PopupMenu>

	<!-- Apply conflicted branch modal -->

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

	<!-- Delete branch confirmation modal -->

	<Modal width="small" bind:this={deleteBranchModal} let:item>
		<svelte:fragment slot="title">Delete branch</svelte:fragment>
		<div>
			Deleting <code>{item.name}</code> cannot be undone.
		</div>
		<svelte:fragment slot="controls" let:close let:item>
			<Button height="small" kind="outlined" on:click={close}>Cancel</Button>
			<Button
				height="small"
				color="destructive"
				on:click={() => {
					branchController.deleteBranch(item.id);
					close();
				}}
			>
				Delete
			</Button>
		</svelte:fragment>
	</Modal>
</div>
