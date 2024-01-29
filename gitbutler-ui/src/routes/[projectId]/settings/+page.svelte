<script lang="ts">
	import CloudForm from './CloudForm.svelte';
	import DetailsForm from './DetailsForm.svelte';
	import KeysForm from './KeysForm.svelte';
	import PreferencesForm from './PreferencesForm.svelte';
	import ScrollableContainer from '../../../lib/components/ScrollableContainer.svelte';
	import Spacer from '../../../lib/components/Spacer.svelte';
	import Button from '$lib/components/Button.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import * as toasts from '$lib/utils/toasts';
	import type { UserError } from '$lib/backend/ipc';
	import type { Key, Project } from '$lib/backend/projects';
	import type { PageData } from './$types';
	import { goto } from '$app/navigation';

	export let data: PageData;

	$: projectService = data.projectService;
	$: project$ = data.project$;
	$: userService = data.userService;
	$: user$ = data.user$;
	$: cloud = data.cloud;

	let deleteConfirmationModal: Modal;
	let isDeleting = false;

	const onDeleteClicked = () =>
		Promise.resolve()
			.then(() => (isDeleting = true))
			.then(() => projectService.deleteProject($project$?.id))
			.catch((e) => {
				console.error(e);
				toasts.error('Failed to delete project');
			})
			.then(() => toasts.success('Project deleted'))
			.then(() => goto('/'))
			.finally(() => {
				isDeleting = false;
				projectService.reload();
			});

	const onKeysUpdated = (e: { detail: { preferred_key: Key } }) =>
		projectService
			.updateProject({ ...$project$, ...e.detail })
			.then(() => toasts.success('Preferred key updated'))
			.catch((e: UserError) => {
				toasts.error(e.message);
			});
	const onCloudUpdated = (e: { detail: Project }) =>
		projectService.updateProject({ ...$project$, ...e.detail });
	const onPreferencesUpdated = (e: { detail: { ok_with_force_push: boolean } }) =>
		projectService.updateProject({ ...$project$, ...e.detail });
	const onDetailsUpdated = async (e: { detail: Project }) => {
		const api =
			$user$ && e.detail.api
				? await cloud.projects.update($user$?.access_token, e.detail.api.repository_id, {
						name: e.detail.title,
						description: e.detail.description
					})
				: undefined;

		projectService.updateProject({
			...e.detail,
			api: api ? { ...api, sync: e.detail.api?.sync || false } : undefined
		});
	};
</script>

<ScrollableContainer wide>
	<div class="settings" data-tauri-drag-region>
		<div class="card">
			{#if !$project$}
				loading...
			{:else}
				<div class="card__header">
					<span class="card_title text-base-16 text-semibold">Project settings</span>
				</div>
				<div class="card__content">
					<CloudForm project={$project$} user={$user$} {userService} on:updated={onCloudUpdated} />
					<Spacer />
					<DetailsForm project={$project$} on:updated={onDetailsUpdated} />
					<Spacer />
					<KeysForm project={$project$} on:updated={onKeysUpdated} />
					<Spacer />
					<PreferencesForm project={$project$} on:updated={onPreferencesUpdated} />
					<Spacer />

					<div class="flex gap-x-4">
						<a
							href="https://discord.gg/wDKZCPEjXC"
							target="_blank"
							rel="noreferrer"
							class="flex-1 rounded border border-light-200 bg-white p-4 dark:border-dark-400 dark:bg-dark-700"
						>
							<p class="mb-2 font-medium">Join our Discord</p>
							<p class="text-light-700 dark:text-dark-200">
								Join our community and share feedback, requests, or ask a question.
							</p>
						</a>
						<a
							href="mailto:hello@gitbutler.com?subject=Feedback or question!"
							target="_blank"
							class="flex-1 rounded border border-light-200 bg-white p-4 dark:border-dark-400 dark:bg-dark-700"
						>
							<p class="mb-2 font-medium">Contact us</p>
							<p class="text-light-700 dark:text-dark-200">
								If you have an issue or any questions, contact us.
							</p>
						</a>
					</div>
				</div>
				<div class="card__footer">
					<Button color="error" kind="outlined" on:click={() => deleteConfirmationModal.show()}>
						Delete project
					</Button>
				</div>
			{/if}
		</div>
	</div>
</ScrollableContainer>

<Modal bind:this={deleteConfirmationModal} title="Delete {$project$?.title}?">
	<p>
		Are you sure you want to delete
		<span class="font-bold">{$project$?.title}</span>? This can’t be undone.
	</p>

	<svelte:fragment slot="controls" let:close>
		<Button kind="outlined" on:click={close}>Cancel</Button>
		<Button color="error" loading={isDeleting} on:click={onDeleteClicked}>Delete project</Button>
	</svelte:fragment>
</Modal>

<style lang="postcss">
	.settings {
		display: flex;
		flex-direction: column;
		padding: var(--space-16) var(--space-16);
		height: 100%;
		width: 100%;
	}
	.card {
		max-width: 50rem;
	}
	.card__content {
		gap: var(--space-24);
	}
</style>
