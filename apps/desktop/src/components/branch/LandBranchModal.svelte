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
	let wholeStack = $state(false);
	let lowerBranches = $state<string[]>([]);

	const targetLabel = $derived(targetBranchName ?? "the target branch");
	const landedLabel = $derived(
		wholeStack ? `"${branchName}" and the branches below it` : `"${branchName}"`,
	);
	const lowerListLabel = $derived(lowerBranches.length > 0 ? ` (${lowerBranches.join(", ")})` : "");

	/**
	 * Passing `stack` lands the whole stack. The intent is carried by the argument itself, not by
	 * `lowerBranches` being non-empty — segments below can be unnamed (deleted branch refs) and
	 * still land.
	 */
	export function show(branch: string, stack?: { lowerBranches: string[] }) {
		branchName = branch;
		wholeStack = stack !== undefined;
		lowerBranches = stack?.lowerBranches ?? [];
		modalEl?.show();
	}

	async function land(): Promise<boolean> {
		if (!branchName) return false;
		try {
			const result = await stackService.landBranch({
				projectId,
				branch: branchName,
				noFf: false,
				wholeStack,
			});
			if (result.landed.type === "alreadyIntegrated") {
				chipToasts.success(`${landedLabel} is already integrated into ${targetLabel}`);
			} else {
				chipToasts.success(`Landed ${landedLabel} into ${targetLabel}`);
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

<Modal bind:this={modalEl} width="small" title={wholeStack ? "Land stack" : "Land branch"}>
	<p>
		{#if wholeStack}
			This lands <strong>{branchName}</strong> and everything below it in its stack{lowerListLabel}
			directly onto {targetLabel}. It cannot be undone.
		{:else}
			This lands <strong>{branchName}</strong> directly onto {targetLabel}. It cannot be undone.
		{/if}
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
