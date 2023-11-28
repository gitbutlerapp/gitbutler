<script lang="ts">
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { User, getCloudApiClient } from '$lib/backend/cloud';
	import type { GitHubIntegrationContext } from '$lib/github/types';
	import BranchCard from './BranchCard.svelte';
	import FileCard from './FileCard.svelte';
	import { writable } from 'svelte/store';
	import { Ownership } from '$lib/vbranches/ownership';
	import type { PrService } from '$lib/github/pullrequest';

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
	export let prService: PrService;

	const selectedOwnership = writable(Ownership.fromBranch(branch));
	const selectedFileId = writable<string | undefined>(undefined);

	function setSelected(fileId: string | undefined, branch: Branch) {
		const match = branch.files?.find((f) => f.id == fileId);
		if (!match) $selectedFileId = undefined;
		return match;
	}

	$: selected = setSelected($selectedFileId, branch);
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
		{selectedFileId}
		{prService}
	/>

	{#if selected}
		<FileCard
			conflicted={selected.conflicted}
			branchId={branch.id}
			file={selected}
			{projectPath}
			{branchController}
			{selectedOwnership}
			selectable={false}
			{readonly}
			on:close={() => selected == undefined}
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
