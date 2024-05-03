<script lang="ts">
	import Button from './Button.svelte';
	import InfoMessage from './InfoMessage.svelte';
	import Select from './Select.svelte';
	import { Project } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import SelectItem from '$lib/components/SelectItem.svelte';
	import Section from '$lib/components/settings/Section.svelte';
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

	let selectedBranch = {name: $baseBranch.branchName};
	let selectedRemote = {name: $baseBranch.actualPushRemoteName()};
	let targetChangeDisabled = true;
	if ($activeBranches) {
		targetChangeDisabled = $activeBranches.length > 0;
	}
	let isSwitching = false;

	function uniqueRemotes(remoteBranches: { name: string }[]) {
		return Array.from(new Set(remoteBranches.map((b) => b.name.split('/')[0]))).map((r) => ({ name: r }));
	}

	async function onSetBaseBranchClick() {
		if (!selectedBranch) return;

		// while target is setting, display loading
		isSwitching = true;

		if(selectedRemote){
			await branchController
				.setTarget(selectedBranch.name, selectedRemote.name)
				.finally(() => {
					isSwitching = false;
				});
		} else {
			await branchController
				.setTarget(selectedBranch.name)
				.finally(() => {
					isSwitching = false;
				});
		}
	}
</script>

<Section spacer>
	<SectionCard labelFor="targetBranch" orientation="column">
		<svelte:fragment slot="title">Current Target Branch</svelte:fragment>
		<svelte:fragment slot="caption">
			Your target branch is what you consider "production". This is where you
			want to integrate any branches that you create. Normally something like
			'origin/master' or 'upstream/main'.
		</svelte:fragment>

		<div class="inputs-group">
			{#if isSwitching}
				<InfoMessage filled outlined={false} style="pop" icon="info">
					<svelte:fragment slot="title">
						Switching target branch...
					</svelte:fragment>
				</InfoMessage>
			{:else}
				{#await getRemoteBranches(project.id)}
					loading remote branches...
				{:then remoteBranches}
					{#if remoteBranches.length == 0}
						<InfoMessage filled outlined={false} style="error" icon="error">
							<svelte:fragment slot="title">
								You don't have any remote branches.
							</svelte:fragment>
						</InfoMessage>
					{:else}
						<div class="inputs-row">
							<Select
								items={remoteBranches}
								bind:value={selectedBranch}
								itemId="name"
								labelId="name"
								disabled={targetChangeDisabled}
								wide={true}
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
								disabled={(selectedBranch.name === $baseBranch.branchName) || targetChangeDisabled}
							>
								Change Target Branch
							</Button>
						</div>

						{#if uniqueRemotes(remoteBranches).length > 1}
							Create branches on remote:
							<Select
								items={uniqueRemotes(remoteBranches)}
								bind:value={selectedRemote}
								itemId="name"
								labelId="name"
								disabled={targetChangeDisabled}
							>
								<SelectItem slot="template" let:item let:selected {selected}>
									{item.name}
								</SelectItem>
							</Select>
						{/if}

					{/if}
				{:catch}
					<InfoMessage filled outlined={true} style="error" icon="error">
						<svelte:fragment slot="title">
							We got an error trying to list your remote branches
						</svelte:fragment>
					</InfoMessage>
				{/await}
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
			{/if}
		</div>
	</SectionCard>
</Section>

<style>
	.inputs-group {
		display: flex;
		flex-direction: column;
		gap: var(--size-16);
		width: 100%;
	}

	.inputs-row {
		display: flex;
		justify-content: space-between;
		gap: var(--size-16);
	}
</style>
