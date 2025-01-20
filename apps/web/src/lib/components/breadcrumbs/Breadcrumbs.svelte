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

	const allProjects = $derived(
		projectSwitcher?.isOpen() && $user?.login
			? getAllUserRelatedProjects(appState, projectService, organizationService, $user.login)
			: undefined
	);

	function openProjectSwitcher() {
		projectSwitcher?.open();
	}
</script>

<div class="flow">
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
			<Button kind="ghost" icon="select-chevron" onclick={openProjectSwitcher}>
				{#if routes.isProjectPageSubset && routes.isProjectPageSubset}
					{routes.isProjectPageSubset.ownerSlug}/{routes.isProjectPageSubset.projectSlug}
				{:else}
					Select project
				{/if}
			</Button>
		</div>

		{#if routes.isProjectReviewPageSubset}
			<p class="text-12 text-semibold grey">/ Branches and Stacks</p>
		{/if}
	{/if}
</div>

<ContextMenu {leftClickTrigger} bind:this={projectSwitcher}>
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
	.flow {
		display: flex;
		gap: 8px;
		flex-wrap: wrap;

		align-items: center;
	}

	.grey {
		color: var(--clr-text-2);
	}
</style>
