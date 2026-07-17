<script lang="ts" module>
	export type ApplyBranchByStackingRequest = {
		incomingName: string;
		ontoName: string;
		onStack: () => Promise<boolean>;
	};
</script>

<script lang="ts">
	import { Button, Modal, TestId } from "@gitbutler/ui";

	let modal = $state<Modal>();
	let request = $state<ApplyBranchByStackingRequest>();
	let loading = $state(false);

	export function show(nextRequest: ApplyBranchByStackingRequest) {
		request = nextRequest;
		modal?.show();
	}

	async function stackBranch(close: () => void) {
		if (!request) return;

		loading = true;
		try {
			if (await request.onStack()) close();
		} finally {
			loading = false;
		}
	}
</script>

<Modal
	testId={TestId.BranchApplyStackingModal}
	bind:this={modal}
	width="small"
	title="Apply branch by stacking?"
	onSubmit={stackBranch}
>
	{#if request}
		<div class="stacking-explanation">
			<p>
				<code>{request.incomingName}</code> conflicts with the applied stack
				<code>{request.ontoName}</code>.
			</p>
			<p>
				Stacking rebases the incoming branch on top of that stack. It may create conflicts to
				resolve on the incoming branch, and a published branch may need to be force-pushed later.
			</p>
		</div>
	{/if}

	{#snippet controls(close)}
		<Button
			testId={TestId.BranchApplyStackingModal_Cancel}
			kind="outline"
			type="reset"
			onclick={close}
			autofocus>Cancel</Button
		>
		<Button
			testId={TestId.BranchApplyStackingModal_ActionButton}
			style="pop"
			type="submit"
			{loading}>Stack branch</Button
		>
	{/snippet}
</Modal>

<style lang="postcss">
	.stacking-explanation {
		display: flex;
		flex-direction: column;
		gap: 12px;
		color: var(--clr-text-2);
		font-size: 13px;
		line-height: 1.45;
	}
</style>
