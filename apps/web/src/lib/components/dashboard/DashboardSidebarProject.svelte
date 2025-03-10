<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { goto } from '$app/navigation';

	type Props = {
		showOwner?: boolean;
		repositoryId: string;
	};

	const { showOwner = false, repositoryId }: Props = $props();

	const routes = getContext(WebRoutesService);

	const project = $derived(getProjectByRepositoryId(repositoryId));
	const projectPageParams = $derived(routes.isProjectPageSubset);
</script>

<Loading loadable={project.current}>
	{#snippet children(project)}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div
			class="project"
			class:current={projectPageParams &&
				projectPageParams.ownerSlug === project.owner &&
				projectPageParams.projectSlug === project.slug}
			onclick={() => {
				goto(routes.projectReviewPath({ ownerSlug: project.owner, projectSlug: project.slug }));
			}}
		>
			<div class="pip"></div>
			<div class="link-container">
				<p class="text-13">{showOwner ? `${project.owner}` : `${project.slug}`}</p>
				<div class="icon">
					<Icon name="chevron-right"></Icon>
				</div>
			</div>
		</div>
	{/snippet}
</Loading>

<style lang="postcss">
	.project {
		display: flex;

		align-items: center;

		gap: 9px;

		&.current {
			.pip {
				background-color: var(--clr-core-pop-50);
			}

			.link-container {
				background-color: var(--clr-theme-pop-bg-muted);
			}

			.icon {
				display: block;
			}
		}
	}

	.pip {
		width: 10px;
		height: 18px;
		border-radius: 5px;
		margin-left: -5px;
	}

	.link-container {
		flex-grow: 1;

		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 10px;

		border-radius: var(--radius-m);

		padding: 10px 14px;
		margin-right: 14px;
	}

	.icon {
		display: none;
		margin-right: -6px;
		height: 16px;
	}
</style>
