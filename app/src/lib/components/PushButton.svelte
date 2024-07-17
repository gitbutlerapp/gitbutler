<script lang="ts" context="module">
	// Disabled because of eslint complaint.
	// `BranchAction` is updated, but is not declared with `$state(...)`
	// eslint-disable-next-line svelte/valid-compile
	export enum BranchAction {
		Push = 'push',
		Integrate = 'integrate'
	}
</script>

<script lang="ts">
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { persisted } from '$lib/persisted/persisted';
	import DropDownButton from '$lib/shared/DropDownButton.svelte';

	const {
		projectId,
		requiresForce,
		isLoading = false,
		canIntegrate = false,
		wide = false,
		trigger
	}: {
		projectId: string;
		requiresForce: boolean;
		isLoading: boolean;
		canIntegrate: boolean;
		wide: boolean;
		trigger: (action: BranchAction) => void;
	} = $props();

	const preferredAction = persisted<BranchAction>(
		BranchAction.Push,
		'projectDefaultAction_' + projectId
	);

	let dropDown: DropDownButton;
	let disabled = false;

	const action = $derived(selectAction($preferredAction));
	const pushLabel = $derived(requiresForce ? 'Force push' : 'Push');
	const labels = $derived({
		[BranchAction.Push]: pushLabel,
		[BranchAction.Integrate]: 'Integrate upstream'
	});

	$effect(() => {
		if (canIntegrate) $preferredAction = BranchAction.Integrate;
	});

	function selectAction(preferredAction: BranchAction) {
		if (preferredAction === BranchAction.Integrate && canIntegrate) return BranchAction.Integrate;
		return BranchAction.Push;
	}
</script>

<DropDownButton
	style="pop"
	kind="solid"
	loading={isLoading}
	bind:this={dropDown}
	{wide}
	{disabled}
	menuPosition="top"
	on:click={() => trigger(action)}
>
	{labels[action]}
	<ContextMenuSection slot="context-menu">
		<ContextMenuItem
			label={labels[BranchAction.Push]}
			on:click={() => {
				$preferredAction = BranchAction.Push;
				dropDown.close();
			}}
		/>
		<ContextMenuItem
			label={labels[BranchAction.Integrate]}
			disabled={!canIntegrate}
			on:click={() => {
				$preferredAction = BranchAction.Integrate;
				dropDown.close();
			}}
		/>
	</ContextMenuSection>
</DropDownButton>
