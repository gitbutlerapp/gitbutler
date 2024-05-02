<script lang="ts">
	import Button from './Button.svelte';
	import InfoMessage from './InfoMessage.svelte';
	import Select from './Select.svelte';
	import { Project } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import SelectItem from '$lib/components/SelectItem.svelte';
	import Spacer from '$lib/components/Spacer.svelte';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { getRemoteBranches } from '$lib/vbranches/baseBranch';
	import { BranchController } from '$lib/vbranches/branchController';
	import { BaseBranch } from '$lib/vbranches/types';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';

	const baseBranch = getContextStore(BaseBranch);
	const vbranchService = getContext(VirtualBranchService);
	const branchController = getContext(BranchController);
	const activeBranches = vbranchService.activeBranches;

	let project = getContext(Project);

	let selectedBranch: {
		name: string;
	} = {
		name: $baseBranch.branchName
	};
	let isSwitching = false;

	async function onSetBaseBranchClick() {
		if (!selectedBranch) return;

		// while target is setting, display loading
		isSwitching = true;

		await branchController
			.setTarget(selectedBranch.name)
			.catch((err) => {
				console.log('error', err);
			})
			.finally(() => {
				isSwitching = false;
			});
	}

	$: console.log('selectedBranch', selectedBranch);
</script>

{#if $activeBranches}
	<SectionCard>
		<svelte:fragment slot="title">Current base branch</svelte:fragment>
		<form class="form-wrapper">
			{#await getRemoteBranches(project.id)}
				loading remote branches...
			{:then remoteBranches}
				<div class="fields-wrapper">
					<Select
						items={remoteBranches}
						bind:value={selectedBranch}
						itemId="name"
						labelId="name"
						selectedItemId={$baseBranch.branchName}
						wide
						disabled={$activeBranches.length > 0}
					>
						<SelectItem slot="template" let:item let:selected {selected}>
							{item.name}
						</SelectItem>
					</Select>

					<Button
						size="cta"
						style="ghost"
						kind="solid"
						on:click={onSetBaseBranchClick}
						id="set-base-branch"
						loading={isSwitching}
						disabled={selectedBranch.name === $baseBranch.branchName}
					>
						Switch branch
					</Button>
				</div>

				{#if $activeBranches.length > 0}
					<InfoMessage filled outlined={false}>
						<svelte:fragment slot="content">
							You have {$activeBranches.length === 1
								? '1 active branch'
								: `${$activeBranches.length} active branches`} in your workspace. Please clear the workspace
							before switching the base branch.
						</svelte:fragment>
					</InfoMessage>
				{/if}
			{/await}
		</form>
	</SectionCard>
	<Spacer />
{/if}

<style>
	.fields-wrapper {
		display: flex;
		gap: var(--size-8);
	}

	.form-wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--size-16);
	}
</style>
