<script lang="ts">
	import { goto } from '$app/navigation';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { CHERRY_APPLY_SERVICE } from '$lib/cherryApply/cherryApplyService';
	import { workspacePath } from '$lib/routes/routes.svelte';
	import { getStackName } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { inject } from '@gitbutler/core/context';
	import { Button, InfoMessage, Modal, RadioButton, SectionCard } from '@gitbutler/ui';

	type Props = {
		projectId: string;
		/** The commit hash to cherry-apply */
		subject?: string;
	};

	let { projectId, subject }: Props = $props();

	const cherryApplyService = inject(CHERRY_APPLY_SERVICE);
	const stackService = inject(STACK_SERVICE);

	let modalRef = $state<Modal>();

	const statusResult = $derived(
		subject ? cherryApplyService.status({ projectId, subject }) : undefined
	);
	const stacksResult = $derived(stackService.stacks(projectId));
	const status = $derived(statusResult?.response);

	let selectedStackId = $state<string | undefined>(undefined);
	const [applyCommit, applyResult] = cherryApplyService.apply();

	$effect(() => {
		if (status?.type === 'lockedToStack') {
			selectedStackId = status.subject;
		}
	});

	export function close() {
		modalRef?.close();
	}

	export function open() {
		modalRef?.show();
	}

	async function handleApply() {
		if (!selectedStackId || !subject) return;

		await applyCommit({
			projectId,
			subject,
			target: selectedStackId
		});

		goto(workspacePath(projectId));

		close();
	}

	function getStatusMessage(): string {
		if (!status) return '';

		switch (status.type) {
			case 'applicableToAnyStack':
				return 'This commit can be applied to any stack. Select a stack below.';
			case 'lockedToStack':
				return 'This commit conflicts when applied to the selected stack, as such it must be applied to the selected stack to avoid a workspace conflict.';
			case 'causesWorkspaceConflict':
				return "This commit can't be applied since it would cause a workspace conflict.";
			case 'noStacks':
				return 'No stacks are currently applied to the workspace.';
		}
	}

	const canApply = $derived(
		status?.type === 'applicableToAnyStack' || status?.type === 'lockedToStack'
	);
	const canSelectStack = $derived(status?.type === 'applicableToAnyStack');
	const isApplying = $derived(applyResult.current.isLoading);

	function handleStackSelectionChange(form: HTMLFormElement) {
		const formData = new FormData(form);
		const selected = formData.get('stackSelection') as string | null;
		if (selected) {
			selectedStackId = selected;
		}
	}

	const messageStyle = $derived(status?.type === 'causesWorkspaceConflict' ? 'warning' : 'info');
</script>

<Modal bind:this={modalRef} title="Cherry-pick commit" width={500}>
	{#if statusResult}
		<ReduxResult {projectId} result={combineResults(statusResult?.result, stacksResult.result)}>
			{#snippet children([_status, stacks], { projectId: _projectId })}
				<div class="cherry-apply-modal">
					<InfoMessage style={messageStyle} outlined filled>
						{#snippet content()}
							{getStatusMessage()}
						{/snippet}
					</InfoMessage>

					{#if canApply && stacks.length > 0}
						<form onchange={(e) => handleStackSelectionChange(e.currentTarget)}>
							{#each stacks as stack, idx (stack.id)}
								{@const isFirst = idx === 0}
								{@const isLast = idx === stacks.length - 1}
								{@const isDisabled = !canSelectStack && selectedStackId !== stack.id}
								<SectionCard
									orientation="row"
									roundedBottom={isLast}
									roundedTop={isFirst}
									labelFor="stack-{stack.id}"
									disabled={isDisabled}
								>
									{#snippet title()}
										{getStackName(stack)}
									{/snippet}
									{#snippet caption()}
										{stack.heads.length}
										{stack.heads.length === 1 ? 'branch' : 'branches'}
									{/snippet}
									{#snippet actions()}
										<RadioButton
											name="stackSelection"
											value={stack.id ?? undefined}
											id="stack-{stack.id}"
											checked={selectedStackId === stack.id}
											disabled={isDisabled}
										/>
									{/snippet}
								</SectionCard>
							{/each}
						</form>
					{/if}
				</div>
			{/snippet}
		</ReduxResult>
	{/if}
	{#snippet controls()}
		<Button kind="outline" onclick={close} disabled={isApplying}>Cancel</Button>
		<Button
			style="pop"
			onclick={handleApply}
			disabled={!canApply || !selectedStackId || isApplying}
			loading={isApplying}
		>
			Apply commit
		</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.cherry-apply-modal {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
</style>
