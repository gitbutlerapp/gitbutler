<script lang="ts">
	import { toasts } from '$lib';
	import type { Commit, File, BaseBranch } from '$lib/vbranches';
	import { getContext, onMount } from 'svelte';
	import { IconAISparkles, IconBranch } from '$lib/icons';
	import { Button, Link, Modal } from '$lib/components';
	import IconMeatballMenu from '$lib/icons/IconMeatballMenu.svelte';
	import CommitCard from './CommitCard.svelte';
	import { getExpandedWithCacheFallback, setExpandedWithCache } from './cache';
	import PopupMenu from '../../../lib/components/PopupMenu/PopupMenu.svelte';
	import PopupMenuItem from '../../../lib/components/PopupMenu/PopupMenuItem.svelte';
	import { dzHighlight } from './dropZone';
	import type { BranchController } from '$lib/vbranches';
	import { BRANCH_CONTROLLER_KEY } from '$lib/vbranches/branchController';
	import FileCardNext from './FileCardNext.svelte';

	export let branchId: string;
	export let projectPath: string;
	export let name: string;
	export let commitMessage: string;
	export let upstream: string | undefined;
	export let files: File[];
	export let commits: Commit[];
	export let projectId: string;
	export let order: number;
	export let conflicted: boolean;
	export let target: BaseBranch;

	const branchController = getContext<BranchController>(BRANCH_CONTROLLER_KEY);

	$: remoteCommits = commits.filter((c) => c.isRemote);
	$: localCommits = commits.filter((c) => !c.isRemote);
	$: messageRows = Math.min(Math.max(commitMessage ? commitMessage.split('\n').length : 0, 1), 10);

	let commitTitle: string;
	let commitDescription: string;
	$: descriptionRows = Math.min(
		Math.max(commitDescription ? commitDescription.split('\n').length : 0, 1),
		10
	);

	let allExpanded: boolean | undefined;
	let maximized = false;
	let isPushing = false;
	let popupMenu: PopupMenu;
	let meatballButton: HTMLButtonElement;
	let textAreaInput: HTMLTextAreaElement;
	let commitTiteInput: HTMLInputElement;
	let descriptionTextArea: HTMLTextAreaElement;
	let commitBranchModal: Modal;

	const hoverClass = 'drop-zone-hover';
	const dzType = 'text/hunk';

	function commit() {
		console.log('commit', commitMessage, projectId, branchId);
		branchController.commitBranch(branchId, commitMessage);
	}

	function push() {
		if (localCommits[0]?.id) {
			console.log(`pushing ${branchId}`);
			isPushing = true;
			branchController.pushBranch(branchId).finally(() => (isPushing = false));
		}
	}

	onMount(() => {
		expandFromCache();
	});

	$: {
		// On refresh we need to check expansion status from localStorage
		files && expandFromCache();
	}

	function expandFromCache() {
		// Exercise cache lookup for all files.
		files.forEach((f) => getExpandedWithCacheFallback(f));
		if (files.every((f) => getExpandedWithCacheFallback(f))) {
			allExpanded = true;
		} else if (files.every((f) => getExpandedWithCacheFallback(f) === false)) {
			allExpanded = false;
		} else {
			allExpanded = undefined;
		}
	}
	function handleToggleExpandAll() {
		if (allExpanded == true) {
			files.forEach((f) => setExpandedWithCache(f, false));
			allExpanded = false;
		} else {
			files.forEach((f) => setExpandedWithCache(f, true));
			allExpanded = true;
		}
		files = files;
	}

	function handleBranchNameChange() {
		console.log('branch name change:', name);
		branchController.updateBranchName(branchId, name);
	}

	function nameToBranch(name: string): string {
		const isAsciiAlphanumeric = (c: string): boolean => /^[A-Za-z0-9]$/.test(c);
		return name
			.split('')
			.map((c) => (isAsciiAlphanumeric(c) ? c : '-'))
			.join('');
	}

	function url(target: BaseBranch, branchName: string) {
		const baseBranchName = target.branchName.split('/')[1];
		const repoBaseUrl = target.remoteUrl
			.replace(':', '/')
			.replace('git@', 'https://')
			.replace('.git', '');
		return `${repoBaseUrl}/compare/${baseBranchName}...${branchName}`;
	}

	function onUpdateFromModal() {
		commitMessage = commitTiteInput.value + '\n\n' + commitDescription;
	}
</script>

<div
	draggable="true"
	class:w-full={maximized}
	class:w-96={!maximized}
	class="lane-scroll flex h-full min-w-[24rem] max-w-[120ch] shrink-0 cursor-default snap-center flex-col overflow-y-scroll overscroll-y-none bg-light-150 pt-2 transition-width dark:bg-dark-1000 dark:text-dark-100"
	role="group"
	use:dzHighlight={{ type: dzType, hover: hoverClass, active: 'drop-zone-active' }}
	on:dragstart
	on:dragend
	on:drop|stopPropagation={(e) => {
		if (!e.dataTransfer) {
			return;
		}
		const data = e.dataTransfer.getData(dzType);
		const [newFileId, newHunks] = data.split(':');
		const existingHunkIds = files.find((f) => f.id === newFileId)?.hunks.map((h) => h.id) || [];
		const newHunkIds = newHunks.split(',').filter((h) => !existingHunkIds.includes(h));
		if (newHunkIds.length == 0) {
			// don't allow dropping hunk to the line where it already is
			return;
		}
		const ownership = files
			.map((file) => file.id + ':' + file.hunks.map((hunk) => hunk.id).join(','))
			.join('\n');
		branchController.updateBranchOwnership(branchId, (data + '\n' + ownership).trim());
	}}
>
	<div
		class="mb-2 flex w-full shrink-0 items-center rounded bg-light-150 px-1 text-light-900 dark:bg-dark-1000 dark:font-normal dark:text-dark-100"
	>
		<div
			on:dblclick={() => (maximized = !maximized)}
			tabindex="0"
			role="button"
			class="flex h-8 w-8 flex-grow-0 items-center justify-center text-light-600 dark:text-dark-200"
		>
			<IconBranch class="h-4 w-4" />
		</div>
		<div class="mr-1 flex-grow">
			<input
				type="text"
				bind:value={name}
				on:change={handleBranchNameChange}
				title={name}
				class="w-full truncate border-0 bg-light-150 font-bold text-light-900 dark:bg-dark-1000 dark:text-dark-100"
			/>
		</div>
		<button
			bind:this={meatballButton}
			class="h-8 w-8 flex-grow-0 p-2 text-light-600 transition-colors hover:bg-zinc-300 dark:text-dark-200 dark:hover:bg-zinc-800"
			on:keydown={() => popupMenu.openByElement(meatballButton, branchId)}
			on:click={() => popupMenu.openByElement(meatballButton, branchId)}
		>
			<IconMeatballMenu />
		</button>
	</div>

	{#if conflicted}
		<div class="mb-2 rounded bg-red-700 p-2 text-white">
			{#if files.some((f) => f.conflicted)}
				This virtual branch conflicts with upstream changes. Please resolve all conflicts and commit
				before you can continue.
			{:else}
				Please commit your resolved conflicts to continue.
			{/if}
		</div>
	{/if}

	<PopupMenu bind:this={popupMenu} let:item={branchId}>
		<PopupMenuItem on:click={() => branchId && branchController.unapplyBranch(branchId)}>
			Unapply
		</PopupMenuItem>

		<PopupMenuItem on:click={handleToggleExpandAll}>
			{#if allExpanded}
				Collapse all
			{:else}
				Expand all
			{/if}
		</PopupMenuItem>

		<div class="mx-3">
			<div class="my-2 h-[0.0625rem] w-full bg-light-300 dark:bg-dark-500" />
		</div>

		<PopupMenuItem on:click={() => branchController.createBranch({ order })}>
			Create branch before
		</PopupMenuItem>

		<PopupMenuItem on:click={() => branchController.createBranch({ order: order + 1 })}>
			Create branch after
		</PopupMenuItem>
	</PopupMenu>

	<div class="flex flex-col pt-2">
		<div class="mb-2 flex items-center">
			<textarea
				bind:this={textAreaInput}
				bind:value={commitMessage}
				on:change={() => {
					commitTitle = commitMessage?.split('\n')?.at(0) || '';
					commitDescription = commitMessage?.split('\n')?.slice(1)?.join('\n').trim() || '';
				}}
				class="shrink-0 flex-grow cursor-text resize-none overflow-x-auto overflow-y-auto border border-white bg-white p-2 font-mono text-dark-700 outline-none hover:border-light-400 focus:border-purple-600 focus:ring-0 dark:border-dark-500 dark:bg-dark-700 dark:text-light-400 dark:hover:border-dark-300"
				placeholder="Your commit message here"
				rows={messageRows}
				required
			/>
		</div>
		<div class="mb-2 mr-2 text-right">
			{#if localCommits.length > 0}
				<Button on:click={push} loading={isPushing} kind="outlined" color="purple" height="small">
					<span class="purple">Push</span>
				</Button>
			{/if}
			<Button height="small" color="purple" on:click={() => commitBranchModal.show()}>
				Commit
			</Button>
		</div>
		<div class="flex flex-shrink flex-col gap-y-2">
			<div class="drop-zone-marker hidden border p-6 text-center">
				Drop here to add to virtual branch
			</div>
			{#each files as file (file.id)}
				<FileCardNext
					expanded={file.expanded}
					conflicted={file.conflicted}
					{file}
					{dzType}
					{projectId}
					{projectPath}
					{maximized}
					on:dblclick={() => (maximized = !maximized)}
					on:expanded={(e) => {
						setExpandedWithCache(file, e.detail);
						expandFromCache();
					}}
				/>
			{/each}
			{#if files.length == 0}
				<!-- attention: these markers have custom css at the bottom of thise file -->
				<div
					class="no-changes rounded p-2 text-center font-mono text-light-700 dark:border-zinc-700"
					data-dnd-ignore
				>
					No uncomitted changes
				</div>
			{/if}
		</div>
	</div>
	<div class="flex h-full">
		<div class="relative z-30 h-full">
			<div
				class="absolute top-0 z-30 ml-[20px] h-full w-px
			bg-gradient-to-b from-transparent via-light-400 dark:via-dark-600
			"
			/>
		</div>
		<div class="z-40 mt-4 flex w-full flex-col gap-2">
			{#each commits.filter((c) => !c.isRemote) as commit (commit.id)}
				<div class="flex w-full items-center pb-2 pr-2">
					<div class="ml-4 w-6">
						<div
							class="h-2.5 w-2.5 rounded-full border-2 border-light-500 bg-light-200 dark:border-dark-500 dark:bg-dark-1000"
						/>
					</div>
					<div class="flex-grow">
						<CommitCard {commit} />
					</div>
				</div>
			{/each}
			{#if remoteCommits.length > 0}
				<div class="ml-12 flex items-center font-mono text-sm">
					<Link target="_blank" rel="noreferrer" href={url(target, nameToBranch(name))}>
						<span class="text-sm font-bold">
							{target.remoteName}/{nameToBranch(name)}
						</span>
					</Link>
				</div>
			{/if}
			{#each commits.filter((c) => c.isRemote) as commit (commit.id)}
				<div class="flex w-full items-center pb-2 pr-2">
					<div class="ml-4 w-6">
						<div
							class="h-2.5 w-2.5 rounded-full border-2 border-light-500 bg-light-500 dark:border-dark-500 dark:bg-dark-500"
							class:bg-light-500={commit.isRemote}
							class:dark:bg-dark-500={commit.isRemote}
						/>
					</div>
					<div class="flex-grow">
						<CommitCard {commit} />
					</div>
				</div>
			{/each}
		</div>
	</div>

	<!-- Commit modal -->

	<Modal icon={IconBranch} bind:this={commitBranchModal}>
		<svelte:fragment slot="title">{name}</svelte:fragment>

		<div class="flex w-full flex-col gap-y-2">
			<div class="flex items-center gap-x-2">
				<input
					bind:this={commitTiteInput}
					bind:value={commitTitle}
					on:change={onUpdateFromModal}
					on:keydown={(e) => {
						if (e.key == 'Enter') descriptionTextArea.focus();
					}}
					class="h-6 shrink-0 flex-grow cursor-text resize-none overflow-x-auto overflow-y-auto rounded border-0 bg-white p-0 font-mono text-xl text-dark-700 outline-none focus:ring-0 dark:bg-dark-1000 dark:text-white"
					placeholder="Your commit title"
					required
				/>
				<Button
					tabindex={-1}
					kind="outlined"
					class="text-light-500"
					height="small"
					icon={IconAISparkles}
					on:click={() => toasts.error('Not implemented yet')}
				/>
			</div>

			<textarea
				bind:this={descriptionTextArea}
				bind:value={commitDescription}
				on:change={onUpdateFromModal}
				class="shrink-0 flex-grow cursor-text resize-none overflow-x-auto overflow-y-auto rounded border-0 bg-white p-0 font-mono text-light-800 outline-none focus:ring-0 dark:bg-dark-1000 dark:text-dark-50"
				placeholder="Your commit message here"
				rows={descriptionRows}
				required
			/>
		</div>
		<svelte:fragment slot="controls" let:close>
			<Button height="small" kind="outlined" on:click={close}>Cancel</Button>
			<Button
				height="small"
				color="purple"
				on:click={() => {
					if (commitMessage) commit();
					close();
				}}
			>
				Commit
			</Button>
		</svelte:fragment>
	</Modal>
</div>
