<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { getRemoteBranches } from '$lib/baseBranch/baseBranchService';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import InfoMessage from '$lib/shared/InfoMessage.svelte';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import Button from '@gitbutler/ui/Button.svelte';

	const baseBranch = getContextStore(BaseBranch);
	const vbranchService = getContext(VirtualBranchService);
	const branchController = getContext(BranchController);
	const activeBranches = vbranchService.branches;

	let project = getContext(Project);

	let selectedBranch = { name: $baseBranch.branchName };
	let selectedRemote = { name: $baseBranch.actualPushRemoteName() };
	let targetChangeDisabled = false;

	if ($activeBranches) {
		targetChangeDisabled = $activeBranches.length > 0;
	}
	let isSwitching = false;

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

{#await getRemoteBranches(project.id)}
	<InfoMessage filled outlined={false} icon="info">
		<svelte:fragment slot="content">Loading remote branches...</svelte:fragment>
	</InfoMessage>
{:then remoteBranches}
	{#if remoteBranches.length > 0}
		<SectionCard>
			<svelte:fragment slot="title">Remote configuration</svelte:fragment>
			<svelte:fragment slot="caption">
				Lets you choose where to push code and set the target branch for contributions. The target
				branch is usually the "production" branch like 'origin/master' or 'upstream/main.' This
				section helps ensure your code goes to the correct remote and branch for integration.
			</svelte:fragment>

			<Select
				value={selectedBranch.name}
				options={remoteBranches.map((b) => ({ label: b.name, value: b.name }))}
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
					options={uniqueRemotes(remoteBranches).map((r) => ({ label: r.name!, value: r.name!}))}
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
					<svelte:fragment slot="content">
						You have {$activeBranches.length === 1
							? '1 active branch'
							: `${$activeBranches.length} active branches`} in your workspace. Please clear the workspace
						before switching the base branch.
					</svelte:fragment>
				</InfoMessage>
			{:else}
				<Button
					size="cta"
					style="ghost"
					outline
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
{:catch}
	<InfoMessage filled outlined={true} style="error" icon="error">
		<svelte:fragment slot="title"
			>We got an error trying to list your remote branches</svelte:fragment
		>
	</InfoMessage>
{/await}
