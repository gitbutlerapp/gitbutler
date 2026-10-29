<script lang="ts">
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button, CardGroup, InfoMessage, Select, SelectItem } from "@gitbutler/ui-svelte";

	const { projectId }: { projectId: string } = $props();

	const stackService = inject(STACK_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const baseBranch = $derived(baseBranchQuery.response);
	const remoteBranchesQuery = $derived(baseBranchService.remoteBranches(projectId));
	const localBranchesQuery = $derived(baseBranchService.localTargetBranches(projectId));
	const localBranches = $derived(localBranchesQuery.response ?? []);
	const currentTarget = $derived(localBranches.find((branch) => branch.isTarget)?.refName);
	const [setBaseBranchTargetRef, targetRefSwitch] = baseBranchService.setTargetRef;

	let selectedBranch = $derived(currentTarget);
	let selectedRemote = $derived(baseBranch?.pushRemoteName ?? undefined);

	const stacksQuery = $derived(stackService.stacks(projectId));
	const stackCount = $derived(stacksQuery.response?.length);
	const targetChangeDisabled = $derived(!!(stackCount && stackCount > 0));

	function uniqueRemotes(remoteBranches: { name: string }[]): { name: string }[] {
		return Array.from(new Set(remoteBranches.map((b) => b.name.split("/")[0])))
			.filter((name): name is string => !!name)
			.map((r) => ({
				name: r,
			}));
	}

	const switching = $derived(targetRefSwitch.current.isLoading);

	async function switchTarget(branch: string, pushRemote?: string) {
		// Only update the target; the user keeps working on their current branch.
		await setBaseBranchTargetRef({ projectId, targetRef: branch, pushRemote });
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

{#if remoteBranchesQuery.result.isLoading || localBranchesQuery.result.isLoading}
	<InfoMessage filled outlined={false} icon="info">
		{#snippet content()}
			Loading branches...
		{/snippet}
	</InfoMessage>
{:else if remoteBranchesQuery.result.isSuccess && localBranchesQuery.result.isSuccess}
	{@const remoteBranches = remoteBranchesQuery.response ?? []}
	{#if localBranches.length > 0}
		{@const remotes = uniqueRemotes(remoteBranches)}
		<CardGroup>
			<CardGroup.Item>
				{#snippet title()}
					Remote configuration
				{/snippet}
				{#snippet caption()}
					Lets you choose where to push code and set the target branch for contributions. The target
					branch is a local branch like 'master' or 'main'. Its configured upstream is used for
					pulling. The publication remote is used for pushing feature branches.
				{/snippet}

				<Select
					value={selectedBranch}
					options={localBranches.map((b) => ({ label: b.name, value: b.refName }))}
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

				{#if remotes.length > 1}
					<Select
						value={selectedRemote}
						options={remotes.map((r) => ({ label: r.name, value: r.name }))}
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
							You have {stackCount === 1 ? "1 active branch" : `${stackCount} active branches`} in your
							workspace. Please clear the workspace before switching the base branch.
						{/snippet}
					</InfoMessage>
				{:else}
					<Button
						kind="outline"
						onclick={onSetBaseBranchClick}
						id="set-base-branch"
						loading={switching}
						disabled={!selectedBranch ||
							(selectedBranch === currentTarget &&
								selectedRemote === (baseBranch?.pushRemoteName ?? undefined)) ||
							targetChangeDisabled}
					>
						{switching ? "Updating target..." : "Update configuration"}
					</Button>
				{/if}
			</CardGroup.Item>
		</CardGroup>
	{/if}
{:else if remoteBranchesQuery.result.isError || localBranchesQuery.result.isError}
	<InfoMessage filled outlined={true} style="danger">
		{#snippet title()}
			We got an error trying to list your branches
		{/snippet}
	</InfoMessage>
{/if}
