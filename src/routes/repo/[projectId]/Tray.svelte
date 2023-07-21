<script lang="ts">
	import { Button, Checkbox, Modal } from '$lib/components';
	import type { Branch, BranchData, BaseBranch } from '$lib/vbranches';
	import { formatDistanceToNow } from 'date-fns';
	import { IconGitBranch, IconRemote } from '$lib/icons';
	import { IconTriangleDown, IconTriangleUp } from '$lib/icons';
	import { accordion } from './accordion';
	import PopupMenu from '$lib/components/PopupMenu/PopupMenu.svelte';
	import PopupMenuItem from '$lib/components/PopupMenu/PopupMenuItem.svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import type { BranchController } from '$lib/vbranches';
	import { getContext } from 'svelte';
	import { BRANCH_CONTROLLER_KEY } from '$lib/vbranches/branchController';

	export let branches: Branch[];
	export let remoteBranches: BranchData[];

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
	const branchController = getContext<BranchController>(BRANCH_CONTROLLER_KEY);

	let yourBranchesOpen = true;
	let remoteBranchesOpen = true;

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
</script>

<div
	class="tray-scroll w-80 min-w-[216px] shrink-0 cursor-default overflow-y-scroll overscroll-y-none border-r border-light-400 bg-white text-light-800 dark:border-dark-600 dark:bg-dark-900 dark:text-dark-100"
	style:width={$userSettings.trayWidth ? `${$userSettings.trayWidth}px` : null}
>
	<!-- Your branches -->
	<div
		class="flex items-center justify-between border-b border-light-400 bg-light-100 px-2 py-1 pr-1 dark:border-dark-600 dark:bg-dark-800"
	>
		<div class="font-bold">Your branches</div>
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
	<div class="flex flex-col dark:bg-dark-900" use:accordion={yourBranchesOpen}>
		{#each branches as branch (branch.id)}
			{@const latestModifiedAt = branch.files.at(0)?.hunks.at(0)?.modifiedAt}
			<div
				role="listitem"
				on:contextmenu|preventDefault={(e) => yourBranchContextMenu.openByMouse(e, branch)}
				class="border-b border-light-400 p-2 dark:border-dark-600"
				title={branch.name}
			>
				<div class="flex flex-row justify-between">
					<div class="flex w-full">
						<Checkbox
							on:change={() => toggleBranch(branch)}
							bind:checked={branch.active}
							disabled={!(branch.mergeable || !branch.baseCurrent) || branch.conflicted}
						/>
						<div class="ml-2 w-full truncate text-black dark:text-white">
							{branch.name}
						</div>
					</div>
					{#if !branch.active}
						{#if !branch.baseCurrent}
							<!-- branch will cause merge conflicts if applied -->
							<div class="text-blue-500">&#9679;</div>
						{:else}
							<div class={branch.mergeable ? 'text-green-500' : 'text-red-500'}>&#9679;</div>
						{/if}
					{/if}
				</div>
				<div class="text-sm text-light-700 dark:text-dark-300">
					{latestModifiedAt ? formatDistanceToNow(latestModifiedAt) : ''}
				</div>
			</div>
		{/each}
	</div>

	<!-- Remote branches -->
	{#if remoteBranches}
		<div
			class="flex items-center justify-between border-b border-light-400 bg-light-100 px-2 py-1 pr-1 dark:border-dark-600 dark:bg-dark-800"
		>
			<div class="font-bold">Remote branches</div>
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

		<div class="dark:bg-dark-900" use:accordion={remoteBranchesOpen}>
			{#each remoteBranches as branch}
				<div
					role="listitem"
					on:contextmenu|preventDefault={(e) => remoteBranchContextMenu.openByMouse(e, branch)}
					class="flex flex-col justify-between gap-1 border-b border-light-400 px-2 py-1 pt-2 dark:border-dark-600"
				>
					<div class="flex flex-row items-center gap-x-2">
						<div class="text-light-600 dark:text-dark-200">
							{#if branch.name.match('refs/remotes')}
								<IconRemote class="h-4 w-4" />
							{:else}
								<IconGitBranch class="h-4 w-4" />
							{/if}
						</div>
						<div class="flex-grow truncate text-black dark:text-white" title={branch.name}>
							{branch.name
								.replace('refs/remotes/', '')
								.replace('origin/', '')
								.replace('refs/heads/', '')}
						</div>
						<div>{branch.ahead}/{branch.behind}</div>
						{#if !branch.mergeable}
							<div class="font-bold text-red-500" title="Can't be merged">!</div>
						{/if}
					</div>
					{#if branch.lastCommitTs > 0}
						<div class="flex flex-row justify-between text-light-700 dark:text-dark-300">
							<div class="text-sm">{formatDistanceToNow(branch.lastCommitTs * 1000)}</div>
							<div
								class="isolate flex -space-x-2 overflow-hidden transition duration-300 ease-in-out hover:space-x-1 hover:transition hover:ease-in"
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
					{/if}
				</div>
			{/each}
		</div>
	{/if}

	<!-- Your branches context menu -->
	<PopupMenu bind:this={yourBranchContextMenu} let:item>
		{@const disabled = branches.some((b) => b.id == item.id && b.active)}
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
