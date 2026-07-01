<script lang="ts">
	import { showError } from "$lib/error/showError";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { AsyncButton, Button, Modal, chipToasts } from "@gitbutler/ui";

	type Props = {
		projectId: string;
		/** Remote-qualified name of the target branch (e.g. origin/master), shown for context. */
		targetBranchName: string | undefined;
	};

	const { projectId, targetBranchName }: Props = $props();
	const stackService = inject(STACK_SERVICE);

	let modalEl = $state<ReturnType<typeof Modal>>();
	let branchName = $state<string>();

	const targetLabel = $derived(targetBranchName ?? "the target branch");

	export function show(branch: string) {
		branchName = branch;
		modalEl?.show();
	}

	async function land(): Promise<boolean> {
		if (!branchName) return false;
		try {
			const result = await stackService.landBranch({ projectId, branch: branchName, noFf: false });
			if (result.landed.type === "alreadyIntegrated") {
				chipToasts.success(`"${branchName}" is already integrated into ${targetLabel}`);
			} else {
				chipToasts.success(`Landed "${branchName}" into ${targetLabel}`);
			}
			if (result.reconcileSkipped) {
				chipToasts.warning("Other branches were left un-reconciled. Run `but pull` to finish.");
			}
			return true;
		} catch (error) {
			showError("Failed to land branch", error);
			return false;
		}
	}
</script>

<Modal bind:this={modalEl} width="small" title="Land branch">
	<p>
		This lands <strong>{branchName}</strong> directly onto {targetLabel}. It cannot be undone.
	</p>
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<AsyncButton
			style="pop"
			action={async () => {
				if (await land()) close();
			}}>Land</AsyncButton
		>
	{/snippet}
</Modal>
