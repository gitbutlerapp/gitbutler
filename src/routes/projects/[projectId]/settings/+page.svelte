<script lang="ts">
	import { derived } from 'svelte/store';
	import { Login } from '$lib/components';
	import type { PageData } from './$types';
	import { log, toasts } from '$lib';
	import { IconRotateClockwise } from '$lib/components/icons';

	export let data: PageData;
	const { project, user, api } = data;

	const repo_id = (url: string) => {
		const hurl = new URL(url);
		const path = hurl.pathname.split('/');
		return path[path.length - 1];
	};

	const hostname = (url: string) => {
		const hurl = new URL(url);
		return hurl.hostname;
	};

	const isSyncing = derived(project, (project) => project?.api?.sync);

	const onSyncChange = async (event: Event) => {
		if ($project === undefined) return;
		if ($user === undefined) return;

		const target = event.target as HTMLInputElement;
		const sync = target.checked;

		try {
			if (!$project.api) {
				const apiProject = await api.projects.create($user.access_token, {
					name: $project.title,
					uid: $project.id
				});
				await project.update({ api: { ...apiProject, sync } });
			} else {
				await project.update({ api: { ...$project.api, sync } });
			}
		} catch (error) {
			target.checked = $project.api?.sync || false;
			log.error(`Failed to update project sync status: ${error}`);
			toasts.error('Failed to update project sync status');
		}
	};

	$: saving = false;
	const onSubmit = async (e: SubmitEvent) => {
		if (!$project) return;
		if (!$user) return;
		saving = true;

		const target = e.target as HTMLFormElement;
		const formData = new FormData(target);
		const name = formData.get('name') as string | undefined;
		const description = formData.get('description') as string | undefined;

		try {
			if (name) {
				const updated = await api.projects.update($user.access_token, $project?.api.repository_id, {
					name,
					description
				});
				await project.update({
					title: name,
					api: { ...updated, sync: $project?.api.sync || false }
				});
			}
			toasts.success('Project updated');
		} catch (e) {
			log.error(e);
			toasts.error('Failed to update project');
		}

		saving = false;
	};
</script>

<div class="mx-auto h-full overflow-auto p-4">
	<div class="mx-auto max-w-2xl p-4">
		<div class="flex flex-col space-y-6">
			<div class="space-y-0">
				<div class="text-xl font-medium">Project Settings</div>
				<div class="">
					How shall I manage your project settings for <strong>{$project?.title}</strong>?
				</div>
			</div>
			<hr class="border-zinc-600" />
			{#if $user}
				<div class="space-y-2">
					<div class="ml-1">GitButler Cloud</div>
					<div
						class="flex flex-row items-center justify-between rounded-lg border border-zinc-600 p-2"
					>
						<div class="flex flex-row space-x-3">
							<svg
								xmlns="http://www.w3.org/2000/svg"
								fill="none"
								viewBox="0 0 24 24"
								stroke-width="1.5"
								stroke="white"
								class="h-6 w-6"
							>
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									d="M12 16.5V9.75m0 0l3 3m-3-3l-3 3M6.75 19.5a4.5 4.5 0 01-1.41-8.775 5.25 5.25 0 0110.233-2.33 3 3 0 013.758 3.848A3.752 3.752 0 0118 19.5H6.75z"
								/>
							</svg>
							<div class="flex flex-row">
								{#if $project?.api?.git_url}
									<div class="flex flex-col">
										<div class="">Git Host</div>
										<div class="font-mono text-zinc-400">
											{hostname($project?.api?.git_url)}
										</div>
										<div class="mt-3 ">Repository ID</div>
										<div class="font-mono text-zinc-400">
											{repo_id($project?.api?.git_url)}
										</div>
									</div>
								{/if}
								<div>
									<form>
										<input
											class="mr-1"
											disabled={$user === undefined}
											type="checkbox"
											checked={$isSyncing}
											on:change={onSyncChange}
										/>
										<label for="sync">Backup your data to GitButler Cloud</label>
									</form>
								</div>
							</div>
						</div>
					</div>
				</div>
			{:else}
				<div class="space-y-2">
					<div class="flex flex-row items-end space-x-2">
						<div class="">GitButler Cloud</div>
						<div class="text-zinc-400">backup your work and access advanced features</div>
					</div>
					<div class="flex flex-row items-center space-x-2">
						<Login {user} {api} />
					</div>
				</div>
			{/if}
			<form on:submit={onSubmit} class="flex flex-col gap-3">
				<fieldset class="flex flex-col gap-2">
					<div class="flex flex-col gap-2">
						<label for="path" class="ml-1">Path</label>
						<input
							disabled
							id="path"
							name="path"
							type="text"
							class="w-full rounded-lg border border-zinc-600 bg-zinc-700 p-2 text-zinc-300"
							value={$project?.path}
						/>
					</div>
					<div class="flex flex-col gap-2">
						<label for="name" class="ml-1">Project Name</label>
						<input
							id="name"
							name="name"
							type="text"
							class="w-full rounded-lg border border-zinc-600 bg-zinc-700 p-2 text-zinc-300"
							value={$project?.title}
							required
						/>
					</div>
					<div class="flex flex-col gap-2">
						<label for="description" class="ml-1">Project Description</label>
						<textarea
							id="description"
							name="description"
							rows="3"
							class="w-full rounded-lg border border-zinc-600 bg-zinc-700 p-2 text-zinc-300"
							value={$project?.api?.description}
						/>
					</div>
				</fieldset>

				<footer>
					{#if saving}
						<div
							class="w-content flex w-32 flex-row items-center justify-center gap-1 rounded bg-blue-400 py-2 px-3 text-white"
						>
							<IconRotateClockwise class="h-5 w-5 animate-spin" />
							<span>Updating...</span>
						</div>
					{:else}
						<button type="submit" class="rounded bg-blue-600 py-2 px-3 text-white cursor-default hover:bg-blue-700"
							>Update profile</button
						>
					{/if}
				</footer>
			</form>
		</div>
	</div>
</div>
