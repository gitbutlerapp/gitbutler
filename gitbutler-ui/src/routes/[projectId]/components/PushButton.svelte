<script lang="ts">
	import { projectCreatePullRequestInsteadOfPush } from '$lib/config/config';
	import { createEventDispatcher } from 'svelte';
	import DropDownButton from '$lib/components/DropDownButton.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';

	let disabled = false;

	export let isLoading = false;
	export let projectId: string;
	export let wide = false;
	export let githubEnabled: boolean;

	const dispatch = createEventDispatcher<{ trigger: { with_pr: boolean } }>();
	const createPr = projectCreatePullRequestInsteadOfPush(projectId);

	let contextMenu: ContextMenu;
	let dropDown: DropDownButton;

	$: selection$ = contextMenu?.selection$;
	$: mode = $createPr && githubEnabled ? 'pr' : 'push';
</script>

<DropDownButton
	color="primary"
	kind="outlined"
	loading={isLoading}
	bind:this={dropDown}
	{wide}
	{disabled}
	on:click={() => {
		dispatch('trigger', { with_pr: $selection$?.id == 'pr' });
	}}
>
	{$selection$?.label}
	<ContextMenu
		type="select"
		slot="context-menu"
		bind:this={contextMenu}
		on:select={(e) => {
			$createPr = e.detail?.id == 'pr';
			dropDown.close();
		}}
	>
		<ContextMenuSection>
			<ContextMenuItem id="push" label="Push to remote" selected={mode == 'push'} />
			<ContextMenuItem
				id="pr"
				label="Create Pull Request"
				disabled={!githubEnabled}
				selected={mode == 'pr'}
			/>
		</ContextMenuSection>
	</ContextMenu>
</DropDownButton>
