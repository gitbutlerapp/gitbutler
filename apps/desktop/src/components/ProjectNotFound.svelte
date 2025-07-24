<script lang="ts">
	import DecorativeSplitView from '$components/DecorativeSplitView.svelte';
	import InfoMessage, { type MessageStyle } from '$components/InfoMessage.svelte';
	import ProjectSwitcher from '$components/ProjectSwitcher.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import RemoveProjectButton from '$components/RemoveProjectButton.svelte';
	import notFoundSvg from '$lib/assets/illustrations/not-found.svg?raw';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';

	interface Props {
		projectId: string;
	}
	const { projectId }: Props = $props();

	const projectsService = inject(PROJECTS_SERVICE);
	const projectResult = $derived(projectsService.getProject(projectId, false));

	let deleteSucceeded: boolean | undefined = $state(undefined);
	let isDeleting = $state(false);

	async function stopTracking(id: string) {
		isDeleting = true;
		deleteProject: {
			try {
				await projectsService.deleteProject(id);
			} catch {
				deleteSucceeded = false;
				break deleteProject;
			}
			deleteSucceeded = true;
		}
		isDeleting = false;
	}

	async function locate(id: string) {
		await projectsService.relocateProject(id);
	}

	interface DeletionStatus {
		message: string;
		style: MessageStyle;
	}

	function getDeletionStatus(repoName: string, deleteSucceeded: boolean): DeletionStatus {
		return deleteSucceeded
			? { message: `Project "${repoName}" successfully deleted`, style: 'success' }
			: { message: `Failed to delete "${repoName}" project`, style: 'error' };
	}
</script>

<DecorativeSplitView testId={TestId.ProjectNotFoundPage} img={notFoundSvg}>
	<div class="container">
		<ReduxResult {projectId} result={projectResult.current}>
			{#snippet children(project)}
				{#if deleteSucceeded === undefined}
					<div class="text-content">
						<h2 class="title-text text-18 text-body text-bold">
							Can’t find "{project.title}"
						</h2>

						<p class="description-text text-13 text-body">
							Sorry, we can't find the project you're looking for.
							<br />
							It might have been removed or doesn't exist.
							<button type="button" class="check-again-btn" onclick={() => location.reload()}
								>Click here</button
							>
							to check again.
							<br />
							The current project path: <span class="code-string">{project.path}</span>
						</p>
					</div>

					<div class="button-container">
						<Button type="button" style="pop" onclick={async () => await locate(projectId)}
							>Locate project…</Button
						>
						<RemoveProjectButton
							noModal
							{isDeleting}
							onDeleteClicked={async () => await stopTracking(project.id)}
						/>
					</div>
				{/if}

				{#if deleteSucceeded !== undefined}
					{@const deletionStatus = getDeletionStatus(project.title, deleteSucceeded)}
					<InfoMessage filled outlined={false} style={deletionStatus.style} icon="info">
						{#snippet content()}
							{deletionStatus.message}
						{/snippet}
					</InfoMessage>
				{/if}
				<div class="text-content">
					<h2 class="title-text text-18 text-body text-bold">Can’t find project</h2>
				</div>
			{/snippet}
		</ReduxResult>

		<Spacer dotted margin={0} />
		<ProjectSwitcher {projectId} />
	</div>
</DecorativeSplitView>

<style lang="postcss">
	.container {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.button-container {
		display: flex;
		gap: 8px;
	}

	.text-content {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.title-text {
		color: var(--clr-scale-ntrl-30);
		/* margin-bottom: 12px; */
	}

	.description-text {
		color: var(--clr-text-2);
		line-height: 1.6;
	}

	.check-again-btn {
		text-decoration: underline;
	}
</style>
