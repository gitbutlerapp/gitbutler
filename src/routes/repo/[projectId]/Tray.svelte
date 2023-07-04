<script lang="ts">
	import { Button, Checkbox, Modal } from '$lib/components';
	import type { Target } from './types';
	import type { Branch } from '$lib/api/ipc/vbranches';
	import { formatDistanceToNow } from 'date-fns';
	import type { VirtualBranchOperations } from './vbranches';
	import { IconGitBranch, IconRemote, IconRefresh, IconAdd } from '$lib/icons';
	import { IconTriangleDown, IconTriangleUp } from '$lib/icons';
	import { accordion } from './accordion';
	import Gravatar from '$lib/components/Gravatar/Gravatar.svelte';
	import PopupMenu from '$lib/components/PopupMenu/PopupMenu.svelte';
	import PopupMenuItem from '$lib/components/PopupMenu/PopupMenuItem.svelte';
	import { getRemoteBranches } from './remoteBranches';
	import { Value } from 'svelte-loadable-store';
	import { get } from 'lscache';
	import { invoke } from '@tauri-apps/api';

	export let target: Target;
	export let branches: Branch[];
	export let projectId: string;
	export let virtualBranches: VirtualBranchOperations;
	const remoteBranchOperations = getRemoteBranches(projectId);
	$: remoteBranches =
		!$remoteBranchOperations.isLoading && !Value.isError($remoteBranchOperations.value)
			? $remoteBranchOperations.value
			: [];

	let yourBranchesOpen = true;
	let remoteBranchesOpen = true;

	let yourBranchContextMenu: PopupMenu;
	let remoteBranchContextMenu: PopupMenu;
	let updateTargetModal: Modal;
	let deleteBranchModal: Modal;

	$: behindMessage = target.behind > 0 ? `behind ${target.behind}` : 'up-to-date';

	function toggleBranch(branchId: string, applied: boolean) {
		if (applied) {
			virtualBranches.unapplyBranch(branchId);
		} else {
			virtualBranches.applyBranch(branchId);
		}
	}

	// store left tray width preference in localStorage
	const cacheKey = 'config:tray-width';

	function makeBranch(branch: string) {
		remoteBranchOperations.createvBranchFromBranch(branch).then(() => {
			virtualBranches.refresh();
		});
	}

	async function getTargetData(params: { projectId: string }) {
		target = await invoke<Target>('get_target_data', params);
	}

	function fetchTarget() {
		remoteBranchOperations.fetchFromTarget().then(() => {
			virtualBranches.refresh();
		});
		getTargetData({ projectId });
	}

	function rememberWidth(node: HTMLElement) {
		const cachedWidth = localStorage.getItem(cacheKey);
		if (cachedWidth) node.style.width = cachedWidth;

		const resizeObserver = new ResizeObserver((entries) => {
			const width = entries.at(0)?.borderBoxSize[0].inlineSize.toString();
			if (width) localStorage.setItem(cacheKey, width + 'px');
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
	class="w-80 shrink-0 resize-x overflow-y-auto bg-white text-light-800 dark:bg-dark-900 dark:text-dark-100"
>
	<!-- Target branch -->
	<div class="pl-2 pr-4 pt-2 text-light-700 dark:bg-dark-700 dark:text-dark-200">Target branch</div>
	<div
		class="flex w-full flex-row items-center gap-x-4 border-b border-light-400 pl-2 pr-4 text-light-900 dark:border-dark-500 dark:bg-dark-700 dark:text-dark-100"
	>
		<div class="flex-grow text-lg font-bold" title={behindMessage}>{target.name}</div>
		<div>{target.behind > 0 ? `behind ${target.behind}` : 'up-to-date'}</div>
		<div class="flex-shrink-0 text-light-700 dark:text-dark-100" title={behindMessage}>
			{#if target.behind == 0}
				<button
					class="p-1 disabled:text-light-300 disabled:dark:text-dark-300"
					on:click={fetchTarget}
					title="click to fetch"
				>
					<IconRefresh />
				</button>
			{:else}
				<button
					class="p-1 disabled:text-light-300 disabled:dark:text-dark-300"
					on:click={updateTargetModal.show}
					disabled={target.behind == 0}
					title={target.behind > 0 ? 'click to update target' : 'already up-to-date'}
				>
					<IconRefresh />
				</button>
			{/if}
		</div>
	</div>

	<!-- Your branches -->
	<div
		class="flex items-center justify-between border-b border-light-400 bg-light-100 py-2 pl-2 pr-4 dark:border-dark-600 dark:bg-dark-800"
	>
		<div class="font-bold">Your branches</div>
		<div>
			<button class="p-1" on:click={() => (yourBranchesOpen = !yourBranchesOpen)}>
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
				class="border-b border-light-400 p-2 pl-2 pr-4 dark:border-dark-600"
				title={branch.name}
			>
				<div class="flex flex-row justify-between">
					<div>
						<Checkbox
							on:change={() => toggleBranch(branch.id, branch.active)}
							bind:checked={branch.active}
						/>
						<span class="ml-2 cursor-pointer text-black dark:text-white">
							{branch.name}
						</span>
					</div>
					{#if !branch.active}
						<div class={branch.mergeable ? 'text-green-500' : 'text-red-500'}>&#9679;</div>
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
			class="flex items-center justify-between border-b border-light-400 bg-light-100 py-2 pl-2 pr-4 dark:border-dark-600 dark:bg-dark-800"
		>
			<div class="font-bold">Remote branches</div>
			<div>
				<button class="p-1" on:click={() => (remoteBranchesOpen = !remoteBranchesOpen)}>
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
					class="flex flex-col justify-between border-b border-light-400 p-2 pl-2 pr-4 dark:border-dark-600"
				>
					<div class="flex flex-row items-center gap-x-2">
						<div>
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
						<div class={branch.mergeable ? 'text-green-500' : 'text-red-500'}>&#9679;</div>
					</div>
					{#if branch.lastCommitTs > 0}
						<div class="flex flex-row justify-between text-light-700 dark:text-dark-300">
							<div class="text-sm">{formatDistanceToNow(branch.lastCommitTs * 1000)}</div>
							<div class="isolate flex -space-x-1 overflow-hidden">
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
		{@const disabled = !remoteBranches.some((b) => b.sha == item.sha && b.mergeable)}
		<PopupMenuItem {disabled} on:click={() => item && makeBranch(item.name)}>Apply</PopupMenuItem>
	</PopupMenu>

	<!-- Confirm target update modal -->

	<Modal width="small" bind:this={updateTargetModal}>
		<p>You are about to update the target branch.</p>
		<svelte:fragment slot="controls" let:close>
			<Button
				height="small"
				color="primary"
				on:click={() => {
					remoteBranchOperations.updateBranchTarget().then(() => {
						virtualBranches.refresh();
					});
					close();
				}}
			>
				Update
			</Button>
		</svelte:fragment>
	</Modal>

	<!-- Delete branch confirmation modal -->

	<Modal width="small" bind:this={deleteBranchModal} let:item>
		<div>
			Are you sure you want to delete <code>{item.name}</code>?
		</div>
		<svelte:fragment slot="controls" let:close let:item>
			<Button
				height="small"
				color="destructive"
				on:click={() => {
					virtualBranches.deleteBranch(item.id).then(() => {
						remoteBranchOperations.refresh();
					});
					close();
				}}
			>
				Delete
			</Button>
		</svelte:fragment>
	</Modal>
</div>
