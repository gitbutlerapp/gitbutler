<script lang="ts">
	import type { File, BaseBranch, Branch } from '$lib/vbranches/types';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { User, getCloudApiClient } from '$lib/backend/cloud';
	import type { GitHubIntegrationContext } from '$lib/github/types';
	import BranchCard from './BranchCard.svelte';
	import FileCard from './FileCard.svelte';
	import { writable } from 'svelte/store';
	import { Ownership } from '$lib/vbranches/ownership';

	export let branch: Branch;
	export let readonly = false;
	export let projectId: string;
	export let base: BaseBranch | undefined | null;
	export let cloudEnabled: boolean;
	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let branchController: BranchController;
	export let maximized = false;
	export let branchCount = 1;
	export let githubContext: GitHubIntegrationContext | undefined;
	export let user: User | undefined;
	export let projectPath: string;

	const selectedOwnership = writable(Ownership.fromBranch(branch));
	const selectedFile = writable<File | undefined>(undefined);
</script>

<div class="wrapper">
	<div class="absolute h-3 w-full" data-tauri-drag-region></div>
	<BranchCard
		{branch}
		{readonly}
		{projectId}
		{base}
		{cloudEnabled}
		{cloud}
		{branchController}
		{maximized}
		{branchCount}
		{githubContext}
		{user}
		{selectedFile}
	/>

	{#if $selectedFile}
		<FileCard
			conflicted={$selectedFile.conflicted}
			branchId={branch.id}
			file={$selectedFile}
			{projectPath}
			{branchController}
			{selectedOwnership}
			selectable={false}
			{readonly}
			on:close={() => ($selectedFile = undefined)}
		/>
	{/if}
</div>

<style lang="postcss">
	.wrapper {
		border: 1px solid var(--clr-theme-container-outline-light);
		display: flex;
		height: 100%;
		flex-shrink: 0;
		border-radius: var(--radius-m);
		overflow: hidden;
	}
</style>
