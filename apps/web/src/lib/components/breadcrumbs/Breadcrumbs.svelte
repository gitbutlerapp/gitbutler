<script lang="ts">
	import { getBreadcrumbsContext } from '$lib/components/breadcrumbs/breadcrumbsContext.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getAllUserRelatedProjects } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';

	const breadcrumbsContext = getBreadcrumbsContext();
	const appState = getContext(AppState);
	const projectService = getContext(ProjectService);
	const organizationService = getContext(OrganizationService);
	const userService = getContext(UserService);
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

<div bind:this={leftClickTrigger}>
	<Button kind="ghost" icon="select-chevron" onclick={openProjectSwitcher}>
		{#if breadcrumbsContext.current.ownerSlug && breadcrumbsContext.current.projectSlug}
			{breadcrumbsContext.current.ownerSlug}/{breadcrumbsContext.current.projectSlug}
		{:else}
			Select project
		{/if}
	</Button>
</div>

<ContextMenu {leftClickTrigger} bind:this={projectSwitcher}>
	<ContextMenuSection>
		{#each allProjects?.current || [] as project}
			<Loading loadable={project}>
				{#snippet children(project)}
					<ContextMenuItem label="{project.owner}/{project.slug}" onclick={() => {}} />
				{/snippet}
			</Loading>
		{/each}
	</ContextMenuSection>
</ContextMenu>
