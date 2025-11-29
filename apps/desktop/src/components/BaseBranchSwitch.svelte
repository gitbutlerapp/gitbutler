<script lang="ts">
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Button, CardGroup, InfoMessage, Select, SelectItem } from '@gitbutler/ui';

	const { projectId }: { projectId: string } = $props();

	const stackService = inject(STACK_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const baseBranch = $derived(baseBranchQuery.response);
	const remoteBranchesQuery = $derived(baseBranchService.remoteBranches(projectId));
	const [setBaseBranchTarget, targetBranchSwitch] = baseBranchService.setTarget;

	let selectedBranch = $derived(baseBranch?.branchName);
	let selectedRemote = $derived(baseBranch?.actualPushRemoteName());

	const stacksQuery = $derived(stackService.stacks(projectId));
	const stackCount = $derived(stacksQuery.response?.length);
	const targetChangeDisabled = $derived(!!(stackCount && stackCount > 0));

	function uniqueRemotes(remoteBranches: { name: string }[]) {
		return Array.from(new Set(remoteBranches.map((b) => b.name.split('/')[0]))).map((r) => ({
			name: r
		}));
	}

	async function switchTarget(branch: string, pushRemote?: string) {
		await setBaseBranchTarget({ projectId, branch, pushRemote });
	}

	async function onSetBaseBranchClick() {
		if (!selectedBranch) return;

		if (selectedRemote) {
			await switchTarget(selectedBranch, selectedRemote);
		} else {
			await switchTarget(selectedBranch);
		}
	}
</script>

{#if remoteBranchesQuery.result.isLoading}
	<InfoMessage filled outlined={false} icon="info">
		{#snippet content()}
			Loading remote branches...
		{/snippet}
	</InfoMessage>
{:else if remoteBranchesQuery.result.isSuccess}
	{@const remoteBranches = remoteBranchesQuery.response}
	{#if remoteBranches && remoteBranches.length > 0}
		<CardGroup>
			<CardGroup.Item>
				{#snippet title()}
					Remote configuration
				{/snippet}
				{#snippet caption()}
					Lets you choose where to push code and set the target branch for contributions. The target
					branch is usually the "production" branch like 'origin/master' or 'upstream/main.' This
					section helps ensure your code goes to the correct remote and branch for integration.
				{/snippet}

				<Select
					value={selectedBranch}
					options={remoteBranches.map((b) => ({ label: b.name, value: b.name }))}
					wide
					onselect={(value) => {
						selectedBranch = value;
					}}
					disabled={targetChangeDisabled}
					label="Current target branch"
					searchable
				>
					{#snippet itemSnippet({ item, highlighted })}
						<SelectItem selected={item.value === selectedBranch} {highlighted}>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>

				{#if uniqueRemotes(remoteBranches).length > 1}
					<Select
						value={selectedRemote}
						options={uniqueRemotes(remoteBranches).map((r) => ({ label: r.name!, value: r.name! }))}
						wide
						onselect={(value) => {
							selectedRemote = value;
						}}
						disabled={targetChangeDisabled}
						label="Create branches on remote"
					>
						{#snippet itemSnippet({ item, highlighted })}
							<SelectItem selected={item.value === selectedRemote} {highlighted}>
								{item.label}
							</SelectItem>
						{/snippet}
					</Select>
				{/if}

				{#if targetChangeDisabled}
					<InfoMessage filled outlined={false} icon="info">
						{#snippet content()}
							You have {stackCount === 1 ? '1 active branch' : `${stackCount} active branches`} in your
							workspace. Please clear the workspace before switching the base branch.
						{/snippet}
					</InfoMessage>
				{:else}
					<Button
						kind="outline"
						onclick={onSetBaseBranchClick}
						id="set-base-branch"
						loading={targetBranchSwitch.current.isLoading}
						disabled={(selectedBranch === baseBranch?.branchName &&
							selectedRemote === baseBranch?.actualPushRemoteName()) ||
							targetChangeDisabled}
					>
						{targetBranchSwitch.current.isLoading
							? 'Switching branches...'
							: 'Update configuration'}
					</Button>
				{/if}
			</CardGroup.Item>
		</CardGroup>
	{/if}
{:else if remoteBranchesQuery.result.isError}
	<InfoMessage filled outlined={true} style="error" icon="error">
		{#snippet title()}
			We got an error trying to list your remote branches
		{/snippet}
	</InfoMessage>
{/if}
