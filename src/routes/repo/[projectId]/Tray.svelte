<script lang="ts">
	import { Button, Checkbox, Modal } from '$lib/components';
	import type { Branch, BranchData, Target } from '$lib/vbranches';
	import { formatDistanceToNow } from 'date-fns';
	import { IconGitBranch, IconRemote, IconRefresh } from '$lib/icons';
	import { IconTriangleDown, IconTriangleUp } from '$lib/icons';
	import { accordion } from './accordion';
	import Gravatar from '$lib/components/Gravatar/Gravatar.svelte';
	import PopupMenu from '$lib/components/PopupMenu/PopupMenu.svelte';
	import PopupMenuItem from '$lib/components/PopupMenu/PopupMenuItem.svelte';
	import type { SettingsStore } from '$lib/userSettings';
	import type { BranchController } from '$lib/vbranches';

	export let target: Target;
	export let branches: Branch[];
	export let branchController: BranchController;
	export let remoteBranches: BranchData[];
	export let userSettings: SettingsStore;

	let yourBranchesOpen = true;
	let remoteBranchesOpen = true;

	let yourBranchContextMenu: PopupMenu;
	let remoteBranchContextMenu: PopupMenu;
	let updateTargetModal: Modal;
	let deleteBranchModal: Modal;

	$: behindMessage = target.behind > 0 ? `behind ${target.behind}` : 'up-to-date';

	function toggleBranch(branchId: string, applied: boolean) {
		if (applied) {
			branchController.unapplyBranch(branchId);
		} else {
			branchController.applyBranch(branchId);
		}
	}

	// store left tray width preference in localStorage
	const cacheKey = 'config:tray-width';

	function rememberWidth(node: HTMLElement) {
		const cachedWidth = localStorage.getItem(cacheKey);
		if (cachedWidth) node.style.width = cachedWidth;

		const resizeObserver = new ResizeObserver((entries) => {
			const width = entries.at(0)?.borderBoxSize[0].inlineSize.toString();
			if (width)
				userSettings.update((s) => ({
					...s,
					trayWidth: width
				}));
		});
		resizeObserver.observe(node);

		return {
			destroy: () => {
				resizeObserver.unobserve(node);
			}
		};
	}
</script>

<div
	use:rememberWidth
	class="w-80 min-w-[216px] max-w-lg shrink-0 resize-x overflow-y-auto border-r border-light-400 bg-white text-light-800 dark:border-dark-600 dark:bg-dark-900 dark:text-dark-100"
>
	<!-- Target branch -->
	<div class="pl-2 pr-4 pt-2 text-light-700 dark:bg-dark-700 dark:text-dark-200">Base branch</div>
	<div
		class="flex w-full flex-row items-center justify-between border-b border-light-400 pb-1 pl-2 pr-1 text-light-900 dark:border-dark-500 dark:bg-dark-700 dark:text-dark-100"
	>
		<div class="flex-grow pb-1 font-bold" title={behindMessage}>{target.branchName}</div>
		<div class="flex items-center gap-1">
			<div class="pb-1">{target.behind > 0 ? `behind ${target.behind}` : 'up-to-date'}</div>
			<div class="flex-shrink-0 text-light-700 dark:text-dark-100" title={behindMessage}>
				{#if target.behind == 0}
					<button
						class="p-0 hover:bg-light-200 disabled:text-light-300 dark:hover:bg-dark-800 disabled:dark:text-dark-300"
						on:click={() => branchController.fetchFromTarget()}
						title="click to fetch"
					>
						<IconRefresh />
					</button>
				{:else}
					<button
						class="p-0 disabled:text-light-300 disabled:dark:text-dark-300"
						on:click={updateTargetModal.show}
						disabled={target.behind == 0}
						title={target.behind > 0 ? 'click to update target' : 'already up-to-date'}
					>
						<IconRefresh />
					</button>
				{/if}
			</div>
		</div>
	</div>

	<!-- Your branches -->
	<div
		class="flex items-center justify-between border-b border-light-400 bg-light-100 py-1 px-2 pr-1 dark:border-dark-600 dark:bg-dark-800"
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
				on:contextmenu|preventDefault={(e) => yourBranchContextMenu.openByMouse(e, branch)}
				class="border-b border-light-400 p-2 dark:border-dark-600"
				title={branch.name}
			>
				<div class="flex flex-row justify-between">
					<div class="flex w-full">
						<Checkbox
							on:change={() => toggleBranch(branch.id, branch.active)}
							bind:checked={branch.active}
							disabled={!(branch.mergeable || !branch.baseCurrent)}
						/>
						<div class="ml-2 w-full cursor-pointer truncate text-black dark:text-white">
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
			class="flex items-center justify-between border-b border-light-400 bg-light-100 py-1 px-2 pr-1 dark:border-dark-600 dark:bg-dark-800"
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
					on:contextmenu|preventDefault={(e) => remoteBranchContextMenu.openByMouse(e, branch)}
					class="flex flex-col justify-between gap-1 border-b border-light-400 py-1 px-2 pt-2 dark:border-dark-600"
				>
					<div class="flex flex-row items-center gap-x-2">
						<div class="text-light-600 dark:text-dark-200">
							{#if branch.branch.match('refs/remotes')}
								<IconRemote class="h-4 w-4" />
							{:else}
								<IconGitBranch class="h-4 w-4" />
							{/if}
						</div>
						<div
							class="flex-grow cursor-pointer truncate text-black dark:text-white"
							title={branch.branch}
						>
							{branch.branch
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
									<Gravatar
										class="relative z-30 inline-block h-4 w-4 rounded-full ring-1 ring-white dark:ring-black"
										email={author}
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

	<!-- Confirm target update modal -->

	<Modal width="small" bind:this={updateTargetModal}>
		<svelte:fragment slot="title">Update target</svelte:fragment>
		<p>You are about to update the target branch.</p>
		<svelte:fragment slot="controls" let:close>
			<Button height="small" kind="outlined" on:click={close}>Cancel</Button>
			<Button
				height="small"
				color="purple"
				on:click={() => {
					branchController.updateBranchTarget();
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
