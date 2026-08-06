<script lang="ts">
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { parseError } from "$lib/error/parser";
	import { showError } from "$lib/error/showError";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { getStackName, handleApplyOutcome } from "$lib/stacks/stack";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		AsyncButton,
		Button,
		CardGroup,
		InfoMessage,
		Modal,
		Select,
		SelectItem,
	} from "@gitbutler/ui";
	import type { Stack } from "$lib/stacks/stack";

	const { projectId }: { projectId: string } = $props();

	const stackService = inject(STACK_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const settingsStore = inject(SETTINGS_SERVICE).appSettings;
	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const baseBranch = $derived(baseBranchQuery.response);
	const remoteBranchesQuery = $derived(baseBranchService.remoteBranches(projectId));
	const [setBaseBranchTarget, targetBranchSwitch] = baseBranchService.setTarget;
	const [setBaseBranchTargetRef, targetRefSwitch] = baseBranchService.setTargetRef;

	let selectedBranch = $derived(baseBranch?.branchName);
	let selectedRemote = $derived(baseBranch?.pushRemoteName);

	const stacksQuery = $derived(stackService.stacks(projectId));
	const stacks = $derived(stacksQuery.response ?? []);
	const stackCount = $derived(stacks.length);

	let confirmModal = $state<Modal>();
	let reapplying = $state(false);

	function uniqueRemotes(remoteBranches: { name: string }[]): { name: string }[] {
		return Array.from(new Set(remoteBranches.map((b) => b.name.split("/")[0])))
			.filter((name): name is string => !!name)
			.map((r) => ({
				name: r,
			}));
	}

	const switching = $derived(
		targetBranchSwitch.current.isLoading || targetRefSwitch.current.isLoading || reapplying,
	);
	// With the singleBranch feature flag, only the target metadata is rewritten
	// and no branch is checked out, so avoid claiming a branch switch.
	const switchingLabel = $derived(
		$settingsStore?.featureFlags.singleBranch ? "Updating target..." : "Switching branches...",
	);

	async function switchTarget(branch: string, pushRemote?: string) {
		if ($settingsStore?.featureFlags.singleBranch) {
			// Only update the target; the user keeps working on their current branch.
			await setBaseBranchTargetRef({ projectId, targetRef: `refs/remotes/${branch}`, pushRemote });
		} else {
			await setBaseBranchTarget({ projectId, branch, pushRemote });
		}
	}

	function errorMessage(error: unknown): string {
		return parseError(error).message;
	}

	/**
	 * Changing the target while branches are applied fails, since the workspace
	 * commit is rebuilt against the currently applied stacks. So we unapply
	 * everything first, switch, then reapply — collecting any failures along
	 * the way instead of aborting, so the user sees the full picture at the end.
	 */
	async function switchTargetWithReapply(branch: string, pushRemote?: string) {
		const stacksToReapply = stacks.filter((stack): stack is Stack & { id: string } => !!stack.id);
		const errors: string[] = [];

		for (const stack of stacksToReapply) {
			try {
				await stackService.unapply({ projectId, stackId: stack.id });
			} catch (error) {
				errors.push(`Failed to unapply "${getStackName(stack)}": ${errorMessage(error)}`);
			}
		}

		try {
			await switchTarget(branch, pushRemote);
		} catch (error) {
			errors.push(`Failed to change the target branch: ${errorMessage(error)}`);
		}

		reapplying = true;
		try {
			for (const stack of stacksToReapply) {
				const branchName = stack.segments.at(0)?.refName?.displayName;
				if (!branchName) {
					errors.push(
						`Could not reapply "${getStackName(stack)}": its top branch has no name to reapply by.`,
					);
					continue;
				}
				try {
					const outcome = await stackService.branchApply({
						projectId,
						existingBranch: `refs/heads/${branchName}`,
					});
					handleApplyOutcome(outcome);
				} catch (error) {
					errors.push(`Failed to reapply "${branchName}": ${errorMessage(error)}`);
				}
			}
		} finally {
			reapplying = false;
		}

		if (errors.length > 0) {
			showError("Some steps failed while switching the base branch", errors.join("\n"));
		}
	}

	async function onSetBaseBranchClick() {
		if (!selectedBranch) return;

		if (stackCount > 0) {
			confirmModal?.show();
			return;
		}

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
		{@const remotes = uniqueRemotes(remoteBranches)}
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
						label="Create branches on remote"
					>
						{#snippet itemSnippet({ item, highlighted })}
							<SelectItem selected={item.value === selectedRemote} {highlighted}>
								{item.label}
							</SelectItem>
						{/snippet}
					</Select>
				{/if}

				<Button
					kind="outline"
					onclick={onSetBaseBranchClick}
					id="set-base-branch"
					loading={switching}
					disabled={selectedBranch === baseBranch?.branchName &&
						selectedRemote === baseBranch?.pushRemoteName}
				>
					{switching ? switchingLabel : "Update configuration"}
				</Button>
			</CardGroup.Item>
		</CardGroup>
	{/if}
{:else if remoteBranchesQuery.result.isError}
	<InfoMessage filled outlined={true} style="danger">
		{#snippet title()}
			We got an error trying to list your remote branches
		{/snippet}
	</InfoMessage>
{/if}

<Modal bind:this={confirmModal} width="small" type="warning" title="Change target branch">
	<p class="text-13 text-body">
		You have {stackCount === 1 ? "1 active branch" : `${stackCount} active branches`} applied in
		your workspace. To switch the target branch, GitButler will unapply all of them, change the
		target, and then reapply them.
	</p>
	<p class="text-13 text-body">
		If reapplying a branch fails (for example due to conflicts with the new target), it is left
		unapplied. Any errors encountered along the way are shown once the operation completes.
	</p>

	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<AsyncButton
			style="warning"
			type="submit"
			action={async () => {
				if (!selectedBranch) return;
				await switchTargetWithReapply(selectedBranch, selectedRemote);
				close();
			}}
		>
			Continue
		</AsyncButton>
	{/snippet}
</Modal>
