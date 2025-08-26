<script lang="ts">
	import InteractiveBranchIntegration from '$components/InteractiveBranchIntegration.svelte';
	import { STACK_SERVICE, type SeriesIntegrationStrategy } from '$lib/stacks/stackService.svelte';
	import { ensureValue } from '$lib/utils/validation';
	import { inject } from '@gitbutler/shared/context';
	import { Button, Modal } from '@gitbutler/ui';

	type Props = {
		modalRef: Modal | undefined;
		projectId: string;
		stackId: string | undefined;
		branchName: string;
	};

	let { modalRef = $bindable(), projectId, stackId, branchName }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const [integrateUpstreamCommits] = stackService.integrateUpstreamCommits;

	let confirmResetModal = $state<Modal>();

	async function integrate(strategy?: SeriesIntegrationStrategy): Promise<void> {
		await integrateUpstreamCommits({
			projectId,
			stackId: ensureValue(stackId),
			seriesName: branchName,
			strategy
		});
	}

	function closeModal() {
		modalRef?.close();
	}
</script>

<Modal bind:this={modalRef} title="Integrate the upstream changes" noPadding width="medium">
	<InteractiveBranchIntegration {projectId} {stackId} {branchName} {closeModal} />
</Modal>

<!-- Confirm hard reset modal -->
<Modal
	bind:this={confirmResetModal}
	title="Reset to remote"
	type="warning"
	width="small"
	onSubmit={async (close) => {
		await integrate('hardreset');
		close();
	}}
>
	<p class="text-13 text-body helper-text">
		This will reset the branch to the state of the remote branch. All local changes will be
		overwritten.
	</p>
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="error" type="submit">Reset</Button>
	{/snippet}
</Modal>
