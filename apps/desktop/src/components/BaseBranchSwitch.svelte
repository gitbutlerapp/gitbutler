<script lang="ts">
	import InfoMessage from '$components/InfoMessage.svelte';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { BranchController } from '$lib/branches/branchController';
	import { VirtualBranchService } from '$lib/branches/virtualBranchService';
	import { Project } from '$lib/project/project';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';

	const project = getContext(Project);
	const projectId = $derived(project.id);
	const baseBranch = getContextStore(BaseBranch);
	const vbranchService = getContext(VirtualBranchService);
	const branchController = getContext(BranchController);
	const baseBranchService = getContext(BaseBranchService);
	const remoteBranchesResponse = $derived(baseBranchService.remoteBranches(projectId));
	const activeBranches = vbranchService.branches;

	let selectedBranch = $state({ name: $baseBranch.branchName });
	let selectedRemote = $state({ name: $baseBranch.actualPushRemoteName() });
	let targetChangeDisabled = $state(false);

	if ($activeBranches) {
		targetChangeDisabled = $activeBranches.length > 0;
	}
	let isSwitching = $state(false);

	function uniqueRemotes(remoteBranches: { name: string }[]) {
		return Array.from(new Set(remoteBranches.map((b) => b.name.split('/')[0]))).map((r) => ({
			name: r
		}));
	}

	async function onSetBaseBranchClick() {
		if (!selectedBranch) return;

		isSwitching = true; // Indicate switching in progress

		if (selectedRemote) {
			await branchController.setTarget(selectedBranch.name, selectedRemote.name).finally(() => {
				isSwitching = false;
			});
		} else {
			await branchController.setTarget(selectedBranch.name).finally(() => {
				isSwitching = false;
			});
		}
	}
</script>

{#if remoteBranchesResponse.current.isLoading}
	<InfoMessage filled outlined={false} icon="info">
		{#snippet content()}
			Loading remote branches...
		{/snippet}
	</InfoMessage>
{:else if remoteBranchesResponse.current.isSuccess}
	{@const remoteBranches = remoteBranchesResponse.current.data}
	{#if remoteBranches.length > 0}
		<SectionCard>
			{#snippet title()}
				Remote configuration
			{/snippet}
			{#snippet caption()}
				Lets you choose where to push code and set the target branch for contributions. The target
				branch is usually the "production" branch like 'origin/master' or 'upstream/main.' This
				section helps ensure your code goes to the correct remote and branch for integration.
			{/snippet}

			<Select
				value={selectedBranch.name}
				options={remoteBranches.map((b) => ({ label: b.name, value: b.name }))}
				wide
				onselect={(value) => {
					selectedBranch = { name: value };
				}}
				disabled={targetChangeDisabled}
				label="Current target branch"
				searchable
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={item.value === selectedBranch.name} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>

			{#if uniqueRemotes(remoteBranches).length > 1}
				<Select
					value={selectedRemote.name}
					options={uniqueRemotes(remoteBranches).map((r) => ({ label: r.name!, value: r.name! }))}
					wide
					onselect={(value) => {
						selectedRemote = { name: value };
					}}
					disabled={targetChangeDisabled}
					label="Create branches on remote"
				>
					{#snippet itemSnippet({ item, highlighted })}
						<SelectItem selected={item.value === selectedRemote.name} {highlighted}>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>
			{/if}

			{#if $activeBranches && targetChangeDisabled}
				<InfoMessage filled outlined={false} icon="info">
					{#snippet content()}
						You have {$activeBranches.length === 1
							? '1 active branch'
							: `${$activeBranches.length} active branches`} in your workspace. Please clear the workspace
						before switching the base branch.
					{/snippet}
				</InfoMessage>
			{:else}
				<Button
					kind="outline"
					onclick={onSetBaseBranchClick}
					id="set-base-branch"
					loading={isSwitching}
					disabled={(selectedBranch.name === $baseBranch.branchName &&
						selectedRemote.name === $baseBranch.actualPushRemoteName()) ||
						targetChangeDisabled}
				>
					{isSwitching ? 'Switching branches...' : 'Update configuration'}
				</Button>
			{/if}
		</SectionCard>
	{/if}
{:else if remoteBranchesResponse.current.isError}
	<InfoMessage filled outlined={true} style="error" icon="error">
		{#snippet title()}
			We got an error trying to list your remote branches
		{/snippet}
	</InfoMessage>
{/if}
