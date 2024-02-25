<script lang="ts">
	import CloudForm from '$lib/components/CloudForm.svelte';
	import DetailsForm from '$lib/components/DetailsForm.svelte';
	import KeysForm from '$lib/components/KeysForm.svelte';
	import PreferencesForm from '$lib/components/PreferencesForm.svelte';
	import RemoveProjectButton from '$lib/components/RemoveProjectButton.svelte';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';
	import Spacer from '$lib/components/Spacer.svelte';
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

	let deleteConfirmationModal: RemoveProjectButton;
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
	const onPreferencesUpdated = (e: {
		detail: { ok_with_force_push?: boolean; omit_certificate_check?: boolean };
	}) => projectService.updateProject({ ...$project$, ...e.detail });
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
					<Spacer margin={2} />
					<DetailsForm project={$project$} on:updated={onDetailsUpdated} />
					<Spacer margin={2} />
					<KeysForm project={$project$} on:updated={onKeysUpdated} />
					<Spacer margin={2} />
					<PreferencesForm project={$project$} on:updated={onPreferencesUpdated} />
					<Spacer margin={2} />

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
					<RemoveProjectButton
						bind:this={deleteConfirmationModal}
						projectTitle={$project$?.title}
						{isDeleting}
						{onDeleteClicked}
					/>
				</div>
			{/if}
		</div>
	</div>
</ScrollableContainer>

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
