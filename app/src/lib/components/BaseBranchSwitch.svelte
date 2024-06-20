<script lang="ts">
	import Button from './Button.svelte';
	import InfoMessage from './InfoMessage.svelte';
	import Select from './Select.svelte';
	import { Project } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import SelectItem from '$lib/components/SelectItem.svelte';
	import Section from '$lib/settings/Section.svelte';
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
		<Section spacer>
			<SectionCard>
				<svelte:fragment slot="title">Remote configuration</svelte:fragment>
				<svelte:fragment slot="caption">
					Lets you choose where to push code and set the target branch for contributions. The target
					branch is usually the "production" branch like 'origin/master' or 'upstream/main.' This
					section helps ensure your code goes to the correct remote and branch for integration.
				</svelte:fragment>

				<Select
					items={remoteBranches}
					bind:value={selectedBranch}
					itemId="name"
					labelId="name"
					selectedItemId={$baseBranch.branchName}
					disabled={targetChangeDisabled}
					wide={true}
					label="Current target branch"
				>
					<SelectItem
						slot="template"
						let:item
						let:selected
						{selected}
						let:highlighted
						{highlighted}
					>
						{item.name}
					</SelectItem>
				</Select>

				{#if uniqueRemotes(remoteBranches).length > 1}
					<Select
						items={uniqueRemotes(remoteBranches)}
						bind:value={selectedRemote}
						itemId="name"
						labelId="name"
						selectedItemId={$baseBranch.actualPushRemoteName()}
						disabled={targetChangeDisabled}
						label="Create branches on remote"
					>
						<SelectItem
							slot="template"
							let:item
							let:selected
							{selected}
							let:highlighted
							{highlighted}
						>
							{item.name}
						</SelectItem>
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
						on:click={onSetBaseBranchClick}
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
		</Section>
	{/if}
{:catch}
	<InfoMessage filled outlined={true} style="error" icon="error">
		<svelte:fragment slot="title"
			>We got an error trying to list your remote branches</svelte:fragment
		>
	</InfoMessage>
{/await}
