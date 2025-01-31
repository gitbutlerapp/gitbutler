<script lang="ts">
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getAllUserRelatedProjects } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import { goto } from '$app/navigation';

	const appState = getContext(AppState);
	const projectService = getContext(ProjectService);
	const organizationService = getContext(OrganizationService);
	const userService = getContext(UserService);
	const routes = getContext(WebRoutesService);
	const user = userService.user;

	let projectSwitcher = $state<ContextMenu>();
	let leftClickTrigger = $state<HTMLElement>();
	let isContextMenuOpen = $state(false);

	const allProjects = $derived(
		projectSwitcher?.isOpen() && $user?.login
			? getAllUserRelatedProjects(appState, projectService, organizationService, $user.login)
			: undefined
	);
</script>

<div class="actions">
	{#if routes.isProjectReviewBranchCommitPageSubset}
		<Button
			kind="ghost"
			icon="chevron-left"
			reversedDirection
			onclick={() => {
				if (!routes.isProjectReviewBranchPageSubset) return;
				goto(routes.projectReviewBranchPath(routes.isProjectReviewBranchPageSubset));
			}}>Back to branch</Button
		>
	{:else if routes.isProjectReviewBranchPageSubset}
		<Button
			kind="ghost"
			icon="chevron-left"
			reversedDirection
			onclick={() => {
				if (!routes.isProjectReviewPageSubset) return;
				goto(routes.projectReviewPath(routes.isProjectReviewPageSubset));
			}}>Back to all branches</Button
		>
	{:else}
		<div bind:this={leftClickTrigger}>
			<Button
				kind="ghost"
				icon="select-chevron"
				onclick={() => projectSwitcher?.toggle()}
				activated={isContextMenuOpen}
			>
				{#if routes.isProjectPageSubset && routes.isProjectPageSubset}
					{routes.isProjectPageSubset.ownerSlug}/{routes.isProjectPageSubset.projectSlug}
				{:else}
					Select project
				{/if}
			</Button>
		</div>

		{#if routes.isProjectReviewPageSubset}
			<div class="text-12 text-semibold current-page-data">
				<span>/</span>
				<span>Branches and Stacks</span>
			</div>
		{/if}
	{/if}
</div>

<ContextMenu
	{leftClickTrigger}
	bind:this={projectSwitcher}
	horizontalAlign="left"
	ontoggle={(isOpen) => (isContextMenuOpen = isOpen)}
>
	<ContextMenuSection>
		{#each allProjects?.current || [] as project}
			<Loading loadable={project}>
				{#snippet children(project)}
					<ContextMenuItem
						label="{project.owner}/{project.slug}"
						onclick={() => {
							goto(routes.projectPath({ ownerSlug: project.owner, projectSlug: project.slug }));
							projectSwitcher?.close();
						}}
					/>
				{/snippet}
			</Loading>
		{/each}
	</ContextMenuSection>
</ContextMenu>

<style lang="postcss">
	.actions {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;

		align-items: center;
	}

	.current-page-data {
		display: flex;
		gap: 10px;
		color: var(--clr-text-2);

		@media (max-width: 800px) {
			display: none;
		}
	}
</style>
