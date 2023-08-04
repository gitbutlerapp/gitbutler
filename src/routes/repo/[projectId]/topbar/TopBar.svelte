<script lang="ts">
	import { IconBranch, IconRemote, IconGithub, IconGitBranch } from '$lib/icons';
	import IconMeatballMenu from '$lib/icons/IconMeatballMenu.svelte';
	import Dropdown from './Dropdown.svelte';
	import Tooltip from '$lib/components/Tooltip/Tooltip.svelte';
	import { formatDistanceToNow } from 'date-fns';
	import type { Branch, BranchData } from '$lib/vbranches/types';
	import PopupMenu from '$lib/components/PopupMenu/PopupMenu.svelte';
	import { Checkbox, Modal, Button } from '$lib/components';
	import PopupMenuItem from '$lib/components/PopupMenu/PopupMenuItem.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';

	export let remoteUrl = '';
	export let branchController: BranchController;
	export let vbranches: Branch[] | undefined;
	$: branches = vbranches ? vbranches.filter((b) => !b.active) : [];
	export let remoteBranches: BranchData[] | undefined;

	let yourBranchContextMenu: PopupMenu;
	let remoteBranchContextMenu: PopupMenu;
	let applyConflictedModal: Modal;
	let deleteBranchModal: Modal;

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
	class="flex items-center border-b border-light-300 bg-light-100 text-sm dark:divide-dark-500 dark:border-dark-500 dark:bg-dark-800"
>
	<div>
		<Dropdown justify="start">
			<div class="flex items-center gap-2" slot="label">
				<IconBranch class="h-3 w-3" />
				{branches.length + ' Stashed branches'}
			</div>
			<div slot="content">
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
								class="h-8 w-8 flex-grow-0 p-2 text-light-600 transition-colors hover:bg-zinc-300 dark:text-dark-200 dark:hover:bg-zinc-800"
								on:click={(e) => yourBranchContextMenu.openByMouse(e, branch)}
							>
								<IconMeatballMenu />
							</button>
						</div>
						<div class="flex items-center text-sm text-light-700 dark:text-dark-300">
							<div class="flex-grow">
								{latestModifiedAt ? formatDistanceToNow(latestModifiedAt) : ''}
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
			</div>
		</Dropdown>
	</div>
	<div>
		<Dropdown>
			<div class="flex items-center gap-2" slot="label">
				{#if remoteUrl.includes('github.com')}
					<IconGithub class="h-3 w-3" />
				{:else}
					<IconRemote class="h-3 w-3" />
				{/if}
				Remote branches
			</div>
			<div slot="content">
				{#each remoteBranches ?? [] as branch}
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
								{formatDistanceToNow(branch.lastCommitTs * 1000)}
							</div>
							<div class="flex flex-grow-0 flex-row space-x-2">
								<Tooltip
									label="This branch has {branch.ahead} commits not on your base branch and your base has {branch.behind} commits not on this branch yet"
								>
									<div class="text-sm">{branch.ahead}/{branch.behind}</div>
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
			</div>
		</Dropdown>
	</div>
	<div>
		<Dropdown disabled={true}>
			<span slot="label" class="text-light-600 dark:text-dark-300">Conflicting branches</span>
		</Dropdown>
	</div>
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
