<script lang="ts">
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import ProjectSwitcher from './ProjectSwitcher.svelte';
	import RemoveProjectButton from './RemoveProjectButton.svelte';
	import notFoundSvg from '$lib/assets/illustrations/not-found.svg?raw';
	import { ProjectService } from '$lib/backend/projects';
	import InfoMessage, { type MessageStyle } from '$lib/shared/InfoMessage.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';

	const projectService = getContext(ProjectService);
	const id = projectService.getLastOpenedProject();
	const projectPromise = id
		? projectService.getProject(id, true)
		: Promise.reject('Failed to get project');

	let deleteSucceeded: boolean | undefined = $state(undefined);
	let isDeleting = $state(false);

	async function stopTracking(id: string) {
		isDeleting = true;
		deleteProject: {
			try {
				await projectService.deleteProject(id);
			} catch {
				deleteSucceeded = false;
				break deleteProject;
			}
			deleteSucceeded = true;
		}
		isDeleting = false;
	}

	async function locate(id: string) {
		await projectService.relocateProject(id);
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

<DecorativeSplitView img={notFoundSvg}>
	<div class="container" data-tauri-drag-region>
		{#await projectPromise then project}
			{#if deleteSucceeded === undefined}
				<div class="text-content">
					<h2 class="title-text text-18 text-body text-bold" data-tauri-drag-region>
						Can’t find "{project.title}"
					</h2>

					<p class="description-text text-13 text-body">
						Sorry, we can't find the project you're looking for.
						<br />
						It might have been removed or doesn't exist.
						<button class="check-again-btn" onclick={() => location.reload()}>Click here</button>
						to check again.
						<br />
						The current project path: <span class="code-string">{project.path}</span>
					</p>
				</div>

				<div class="button-container">
					<Button
						type="button"
						style="pop"
						kind="solid"
						onclick={async () => await locate(project.id)}>Locate project…</Button
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
					<svelte:fragment slot="content">
						{deletionStatus.message}
					</svelte:fragment>
				</InfoMessage>
			{/if}
		{:catch}
			<div class="text-content">
				<h2 class="title-text text-18 text-body text-bold">Can’t find project</h2>
			</div>
		{/await}

		<Spacer dotted margin={0} />
		<ProjectSwitcher />
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
