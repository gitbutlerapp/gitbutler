<!--
	@component

	The button used to create different kinds of reviews, intended to be
	shared between a couple of different kinds of headers for branches.
-->
<script module lang="ts">
	// TODO: Why does eslint complain about this line?
	// eslint-disable-next-line svelte/valid-compile
	export enum Action {
		CreateButlerReview = 'Create Butler Review',
		CreatePullRequest = 'Create Pull Request'
	}
</script>

<script lang="ts">
	import { persisted } from '@gitbutler/shared/persisted';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';

	type Props = {
		loading?: boolean;
		disabled?: boolean;
		onclick: (action: Action) => void;
	};

	const { loading, disabled, onclick }: Props = $props();

	let dropDown = $state<ReturnType<typeof DropDownButton>>();
	const action = persisted<Action>(Action.CreateButlerReview, 'defaultCreateAction');
</script>

<DropDownButton
	bind:this={dropDown}
	kind="outline"
	disabled={loading || disabled}
	{loading}
	type="submit"
	autoClose
	onclick={() => onclick($action)}
>
	{$action}…
	{#snippet contextMenuSlot()}
		<ContextMenuSection>
			<ContextMenuItem
				label={Action.CreateButlerReview}
				onclick={() => {
					$action = Action.CreateButlerReview;
				}}
			/>
			<ContextMenuItem
				label={Action.CreatePullRequest}
				onclick={() => {
					$action = Action.CreatePullRequest;
				}}
			/>
		</ContextMenuSection>
	{/snippet}
</DropDownButton>

<style lang="postcss">
</style>
