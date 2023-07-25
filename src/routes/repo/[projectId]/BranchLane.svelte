<script lang="ts">
	import { toasts, stores } from '$lib';
	import type { Commit, File, BaseBranch } from '$lib/vbranches';
	import { getContext, onMount } from 'svelte';
	import { IconAISparkles } from '$lib/icons';
	import { Button, Link, Tooltip } from '$lib/components';
	import IconMeatballMenu from '$lib/icons/IconMeatballMenu.svelte';
	import CommitCard from './CommitCard.svelte';
	import { getExpandedWithCacheFallback, setExpandedWithCache } from './cache';
	import PopupMenu from '../../../lib/components/PopupMenu/PopupMenu.svelte';
	import PopupMenuItem from '../../../lib/components/PopupMenu/PopupMenuItem.svelte';
	import { dzHighlight } from './dropZone';
	import type { BranchController } from '$lib/vbranches';
	import { BRANCH_CONTROLLER_KEY } from '$lib/vbranches/branchController';
	import FileCardNext from './FileCardNext.svelte';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { crossfade, fade } from 'svelte/transition';
	import { flip } from 'svelte/animate';
	import { invoke } from '@tauri-apps/api/tauri';
	import type { CloudApi } from '$lib/api';

	const [send, receive] = crossfade({
		duration: (d) => Math.sqrt(d * 200),

		fallback(node) {
			const style = getComputedStyle(node);
			const transform = style.transform === 'none' ? '' : style.transform;

			return {
				duration: 600,
				easing: quintOut,
				css: (t) => `
					transform: ${transform} scale(${t});
					opacity: ${t}
				`
			};
		}
	});

	export let branchId: string;
	export let projectPath: string;
	export let name: string;
	export let files: File[];
	export let commits: Commit[];
	export let projectId: string;
	export let order: number;
	export let conflicted: boolean;
	export let base: BaseBranch | undefined;
	export let cloudEnabled: boolean;
	export let cloud: ReturnType<typeof CloudApi>;
	export let upstream: string | undefined;

	const branchController = getContext<BranchController>(BRANCH_CONTROLLER_KEY);
	const user = stores.user;
	let commitMessage: string;

	$: remoteCommits = commits.filter((c) => c.isRemote);
	$: localCommits = commits.filter((c) => !c.isRemote);
	$: messageRows = Math.min(Math.max(commitMessage ? commitMessage.split('\n').length : 0, 1), 10);

	let allExpanded: boolean | undefined;
	let maximized = false;
	let isPushing = false;
	let popupMenu: PopupMenu;
	let meatballButton: HTMLButtonElement;
	let textAreaInput: HTMLTextAreaElement;

	const hoverClass = 'drop-zone-hover';
	const dzType = 'text/hunk';

	function commit() {
		branchController.commitBranch(branchId, commitMessage);
	}

	function push() {
		if (localCommits[0]?.id) {
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
		branchController.updateBranchName(branchId, name);
	}

	function url(target: BaseBranch | undefined, upstreamBranchName: string) {
		if (!target) return undefined;
		const baseBranchName = target.branchName.split('/')[1];
		const parts = upstreamBranchName.split('/');
		const branchName = parts[parts.length - 1];
		return `${target.repoBaseUrl}/compare/${baseBranchName}...${branchName}`;
	}

	let commitDialogShown = false;

	$: if (commitDialogShown && files.length === 0) {
		commitDialogShown = false;
	}

	export function git_get_config(params: { key: string }) {
		return invoke<string>('git_get_global_config', params);
	}

	let annotateCommits = true;

	function checkCommitsAnnotated() {
		git_get_config({ key: 'gitbutler.utmostDiscretion' }).then((value) => {
			annotateCommits = value ? value === '0' : true;
		});
	}
	$: checkCommitsAnnotated();

	let isGeneratingCommigMessage = false;
	function trimNonLetters(input: string): string {
		const regex = /^[^a-zA-Z]+|[^a-zA-Z]+$/g;
		const trimmedString = input.replace(regex, '');
		return trimmedString;
	}
	async function generateCommitMessage(files: File[]) {
		const diff = files
			.map((f) => f.hunks)
			.flat()
			.map((h) => h.diff)
			.flat()
			.join('\n')
			.slice(0, 5000);

		if ($user === null) return;

		isGeneratingCommigMessage = true;
		cloud.summarize
			.commit($user.access_token, {
				diff,
				uid: projectId
			})
			.then(({ message }) => {
				const firstNewLine = message.indexOf('\n');
				const summary = firstNewLine > -1 ? message.slice(0, firstNewLine).trim() : message;
				const description = firstNewLine > -1 ? message.slice(firstNewLine + 1).trim() : '';
				commitMessage = trimNonLetters(description.length > 0 ? `${summary}\n\n${description}` : summary);
			})
			.catch(() => {
				toasts.error('Failed to generate commit message');
			})
			.finally(() => {
				isGeneratingCommigMessage = false;
			});
	}
</script>

<div
	draggable="true"
	class:w-full={maximized}
	class:w-96={!maximized}
	class="flex h-full min-w-[24rem] max-w-[120ch] shrink-0 cursor-default snap-center flex-col bg-light-150 transition-width dark:bg-dark-1000 dark:text-dark-100"
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
	on:dblclick={() => (maximized = !maximized)}
>
	<div
		class="flex w-full shrink-0 flex-col items-center border-b border-r border-light-400 bg-light-200 text-light-900 dark:border-dark-600 dark:bg-dark-800 dark:font-normal dark:text-dark-100"
	>
		<div class="flex w-full items-center py-1 pl-2 pr-4">
			<button
				bind:this={meatballButton}
				class="h-8 w-8 flex-grow-0 p-2 text-light-600 transition-colors hover:bg-zinc-300 dark:text-dark-200 dark:hover:bg-zinc-800"
				on:keydown={() => popupMenu.openByElement(meatballButton, branchId)}
				on:click={() => popupMenu.openByElement(meatballButton, branchId)}
			>
				<IconMeatballMenu />
			</button>
			<div class="flex-grow">
				<input
					type="text"
					bind:value={name}
					on:change={handleBranchNameChange}
					title={name}
					class=" w-full truncate border-0 bg-light-200 font-mono font-bold text-light-800 focus:ring-0 dark:bg-dark-800 dark:text-dark-100"
					on:dblclick|stopPropagation
				/>
			</div>
			<div class:invisible={files.length == 0} transition:fade={{ duration: 150 }}>
				<Button
					class="w-20"
					height="small"
					kind="outlined"
					color="purple"
					disabled={files.length == 0}
					on:click={() => (commitDialogShown = !commitDialogShown)}
				>
					<span class="purple">
						{#if !commitDialogShown}
							Commit
						{:else}
							Cancel
						{/if}
					</span>
				</Button>
			</div>
		</div>

		{#if commitDialogShown}
			<div
				class="flex w-full flex-col border-t border-light-400 bg-light-200 dark:border-dark-400 dark:bg-dark-800"
				transition:slide={{ duration: 150 }}
			>
				{#if annotateCommits}
					<div class="bg-blue-400 p-2 text-sm text-white">
						GitButler will be the "committer" of this commit.
						<a
							target="_blank"
							rel="noreferrer"
							class="font-bold"
							href="https://docs.gitbutler.com/features/virtual-branches/committer-mark"
							>Learn more</a
						>
					</div>
				{/if}
				<div class="flex items-center">
					<textarea
						bind:this={textAreaInput}
						bind:value={commitMessage}
						on:dblclick|stopPropagation
						class="shrink-0 flex-grow cursor-text resize-none overflow-x-auto overflow-y-auto border border-white bg-white p-2 font-mono text-dark-700 outline-none focus:border-purple-600 focus:ring-0 dark:border-dark-500 dark:bg-dark-700 dark:text-light-400"
						placeholder="Your commit message here"
						rows={messageRows}
						required
					/>
				</div>
				<div class="flex flex-grow justify-end gap-2 p-3 px-5">
					<div>
						{#if cloudEnabled && $user}
							<Button
								disabled={isGeneratingCommigMessage}
								tabindex={-1}
								kind="outlined"
								class="text-light-500"
								height="small"
								icon={IconAISparkles}
								loading={isGeneratingCommigMessage}
								on:click={() => generateCommitMessage(files)}
							>
								<span class="text-light-700">Generate message</span>
							</Button>
						{:else}
							<Tooltip
								label="Summary generation requres that you are logged in and have cloud sync enabled for the project"
							>
								<Button
									disabled={true}
									tabindex={-1}
									kind="outlined"
									class="text-light-500"
									height="small"
									icon={IconAISparkles}
									loading={isGeneratingCommigMessage}
								>
									<span class="text-light-700">Generate message</span>
								</Button>
							</Tooltip>
						{/if}
					</div>
					<Button
						class="w-20"
						height="small"
						color="purple"
						on:click={() => {
							if (commitMessage) commit();
							commitDialogShown = false;
						}}
					>
						Commit
					</Button>
				</div>
			</div>
		{/if}
	</div>

	<div class="lane-scroll flex flex-grow flex-col overflow-y-scroll pb-8">
		{#if conflicted}
			<div class="mb-2 bg-red-500 p-2 font-bold text-white">
				{#if files.some((f) => f.conflicted)}
					This virtual branch conflicts with upstream changes. Please resolve all conflicts and
					commit before you can continue.
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

		<div class="flex flex-col py-2">
			<div class="drop-zone-marker hidden border p-6 text-center">
				Drop here to add to virtual branch
			</div>
			{#if files.length > 0}
				<div class="flex flex-shrink flex-col gap-y-2" transition:slide={{ duration: 150 }}>
					{#each files as file (file.id)}
						<FileCardNext
							expanded={file.expanded}
							conflicted={file.conflicted}
							{file}
							{dzType}
							{projectId}
							{projectPath}
							on:expanded={(e) => {
								setExpandedWithCache(file, e.detail);
								expandFromCache();
							}}
						/>
					{/each}
				</div>
			{/if}
			{#if files.length == 0}
				<!-- attention: these markers have custom css at the bottom of thise file -->
				<div
					class="no-changes rounded text-center font-mono text-light-700 dark:border-zinc-700"
					data-dnd-ignore
				>
					No uncomitted changes
				</div>
			{/if}
		</div>
		<div
			class="flex w-full flex-grow flex-col gap-2 border-t border-light-400 dark:border-dark-500"
		>
			{#if localCommits.length > 0}
				<div
					class="relative"
					class:flex-grow={remoteCommits.length == 0}
					transition:slide={{ duration: 150 }}
				>
					<div
						class="dark:form-dark-600 absolute top-4 ml-[1.0625rem] w-px bg-gradient-to-b from-light-400 via-light-500 via-90% dark:from-dark-600 dark:via-dark-600"
						style={remoteCommits.length == 0 ? 'height: calc(100% - 1rem);' : 'height: 100%;'}
					/>

					<div class="relative flex flex-col gap-2">
						<div
							class="dark:form-dark-600 absolute top-4 ml-[1.0625rem] h-px w-6 bg-gradient-to-r from-light-400 via-light-400 via-10% dark:from-dark-600 dark:via-dark-600"
						/>
						<div class="ml-10 mr-2 flex items-center py-2">
							<div
								class="ml-2 flex-grow font-mono text-sm font-bold text-dark-300 dark:text-light-300"
							>
								local
							</div>
							<Button
								class="w-20"
								height="small"
								kind="outlined"
								color="purple"
								loading={isPushing}
								on:click={push}
							>
								<span class="purple">Push</span>
							</Button>
						</div>

						{#each localCommits as commit (commit.id)}
							<div
								class="flex w-full items-center pb-2 pr-2"
								in:receive={{ key: commit.id }}
								out:send={{ key: commit.id }}
								animate:flip
							>
								<div class="ml-[0.875rem] mr-2">
									<div
										class="h-3 w-3 rounded-full border-2 border-light-500 bg-light-200 dark:border-dark-600 dark:bg-dark-1000"
									/>
								</div>
								<CommitCard {commit} {base} />
							</div>
						{/each}
					</div>
				</div>
			{/if}
			{#if remoteCommits.length > 0}
				<div class="relative flex-grow">
					<div
						class="dark:form-dark-600 absolute top-4 ml-[1.0625rem]
						w-px bg-gradient-to-b from-light-600 via-light-600 via-90% dark:from-dark-400 dark:via-dark-400"
						style="height: calc(100% - 1rem);"
					/>

					<div class="relative flex flex-col gap-2">
						<div
							class="dark:form-dark-600 absolute top-4 ml-[1.0625rem] h-px w-6 bg-gradient-to-r from-light-600 via-light-600 via-10% dark:from-dark-400 dark:via-dark-400"
						/>

						<div class="ml-12 flex items-center py-2 font-mono text-sm">
							{#if upstream}
								<Link target="_blank" rel="noreferrer" href={url(base, upstream)}>
									<span class="text-sm font-bold">
										{upstream.split('refs/remotes/')[1]}
									</span>
								</Link>
							{/if}
						</div>

						{#each remoteCommits as commit (commit.id)}
							<div
								class="flex w-full items-center pb-2 pr-2"
								in:receive={{ key: commit.id }}
								out:send={{ key: commit.id }}
								animate:flip
							>
								<div class="ml-3 mr-2">
									<div
										class="h-3 w-3 rounded-full border-2 border-light-600 bg-light-600 dark:border-dark-400 dark:bg-dark-400"
										class:bg-light-500={commit.isRemote}
										class:dark:bg-dark-500={commit.isRemote}
									/>
								</div>
								<CommitCard {commit} {base} />
							</div>
						{/each}
					</div>
				</div>
			{/if}
		</div>
	</div>
</div>
