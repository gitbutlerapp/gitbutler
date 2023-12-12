<script lang="ts">
	import { projectCreatePullRequestInsteadOfPush } from '$lib/config/config';
	import type { GitHubIntegrationContext } from '$lib/github/types';
	import { createEventDispatcher } from 'svelte';
	import DropDown from '$lib/components/DropDown.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';

	let disabled = false;

	export let isLoading = false;
	export let projectId: string;
	export let wide = false;
	export let githubContext: GitHubIntegrationContext | undefined;

	const dispatch = createEventDispatcher<{ trigger: { with_pr: boolean } }>();
	const createPr = projectCreatePullRequestInsteadOfPush(projectId);

	let contextMenu: ContextMenu;
	let dropDown: DropDown;

	$: selection$ = contextMenu?.selection$;
	$: mode = $createPr && githubContext ? 'pr' : 'push';
</script>

<DropDown
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
				disabled={!githubContext}
				selected={mode == 'pr'}
			/>
		</ContextMenuSection>
	</ContextMenu>
</DropDown>
