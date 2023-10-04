<script async lang="ts">
	import Button from '$lib/components/Button/Button.svelte';
	import IconExternalLink from '$lib/icons/IconExternalLink.svelte';
	import IconLoading from '$lib/icons/IconLoading.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import { getRemoteBranches } from '$lib/vbranches/branchStoresCache';

	export let projectId: string;
	export let branchController: BranchController;

	let targetChoice: string | undefined;
	let loading = false;

	const remoteBranchNames = getRemoteBranches(projectId);

	function onSetTargetClick() {
		if (!targetChoice) {
			return;
		}
		loading = true;
		branchController.setTarget(targetChoice).finally(() => (loading = false));
	}
</script>

<div class="grid h-full w-full grid-cols-2 items-center justify-items-stretch">
	<div
		id="vb-data"
		class="bg-color-3 flex h-full flex-col justify-center gap-y-4 self-center p-12 text-lg"
	>
		<div class="font-bold">Set your Base Branch</div>
		<p class="text-color-3">
			You need to set your base branch before you can start working on your project.
		</p>
		<!-- select menu of remoteBranches -->
		{#await remoteBranchNames}
			<IconLoading class="animate-spin fill-blue-600 text-light-600"></IconLoading>
		{:then names}
			{#if names.length == 0}
				<p class="mt-6 text-red-500">You don't have any remote branches.</p>
				<p class="text-color-3 mt-6 text-sm">
					Currently, GitButler requires a remote branch to base it's virtual branch work on. To use
					virtual branches, please push your code to a remote branch to use as a base.
					<a
						target="_blank"
						rel="noreferrer"
						class="font-bold"
						href="https://docs.gitbutler.com/features/virtual-branches/butler-flow">Learn more</a
					>
				</p>
			{:else}
				<select bind:value={targetChoice} disabled={loading}>
					{#each names
						.map((name) => name.substring(13))
						.sort((a, b) => a.localeCompare(b)) as branch}
						{#if branch == 'origin/master' || branch == 'origin/main'}
							<option value={branch} selected>{branch}</option>
						{:else}
							<option value={branch}>{branch}</option>
						{/if}
					{/each}
				</select>
				<p class="text-color-3 text-base">
					This is the branch that you consider "production", normally something like "origin/master"
					or "origin/main".
				</p>
				<div>
					<Button
						color="purple"
						height="small"
						{loading}
						on:click={onSetTargetClick}
						id="set-base-branch">Set Base Branch</Button
					>
				</div>
			{/if}
		{:catch}
			<p class="text-sm text-red-500">Could not load remote branch names</p>
		{/await}
	</div>
	<div id="vb-data" class="max-h-full justify-center overflow-y-auto">
		<div class="flex h-full max-h-full flex-col gap-y-3 p-12 text-lg">
			<h1 class="text-xl font-bold">Getting Started with Virtual Branches</h1>
			<p class="text-color-3 text-xl">
				Virtual branches are just like normal Git branches, except that you can work on several of
				them at the same time.
			</p>
			<div class="font-bold">Base Branch</div>
			<p class="text-color-3">
				With virtual branches, you are not working off of local main or master branches. Everything
				that you do is on a virtual branch, automatically.
			</p>
			<p class="text-color-3">
				This works by specifying a "base branch" that represents the state of production, normally
				something like "origin/master".
			</p>
			<div class="font-bold">Ownership, Committing and Pushing</div>
			<p class="text-color-3">
				Each virtual branch "owns" parts of the files that are seen as changed. If you commit on
				that branch, only the parts that are owned by that branch are actually recorded in the
				commits on that branch.
			</p>
			<p class="text-color-3">
				When you push a virtual branch, it will create a branch name based on your branch title,
				push that branch to your remote with just the changes committed to that branch, not
				everything in your working directory.
			</p>
			<div class="font-bold">Applying and Unapplying</div>
			<p class="text-color-3">
				You can have many virtual branches applied at the same time, but they cannot conflict with
				each other currently. Unapplying a virtual branch will take all of the changes that it owns
				and remove them from your working directory. Applying the branch will add those changes back
				in.
			</p>
			<div class="flex flex-row place-content-center content-center space-x-2 pt-4 text-blue-600">
				<a
					target="_blank"
					rel="noreferrer"
					class="font-bold"
					href="https://docs.gitbutler.com/features/virtual-branches">Learn more</a
				>
				<IconExternalLink class="h-4 w-4" />
			</div>
		</div>
	</div>
</div>
