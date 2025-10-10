<script lang="ts">
	import { goto } from '$app/navigation';
	import Login from '$components/Login.svelte';
	import WelcomeSigninAction from '$components/WelcomeSigninAction.svelte';
	import CliSymLink from '$components/profileSettings/CliSymLink.svelte';
	import { CLI_MANAGER } from '$lib/cli/cli';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { showError } from '$lib/notifications/toasts';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { SETTINGS, type CodeEditorSettings } from '$lib/settings/userSettings';
	import { UPDATER_SERVICE } from '$lib/updater/updater';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import {
		Button,
		Modal,
		ProfilePictureUpload,
		SectionCard,
		Select,
		SelectItem,
		Spacer,
		Textbox,
		Toggle,
		chipToasts
	} from '@gitbutler/ui';
	import type { User } from '$lib/user/user';

	const userService = inject(USER_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const projectsService = inject(PROJECTS_SERVICE);
	const user = userService.user;

	const updaterService = inject(UPDATER_SERVICE);
	const disableAutoChecks = updaterService.disableAutoChecks;

	const cliManager = inject(CLI_MANAGER);
	const [instalCLI, installingCLI] = cliManager.install;

	let saving = $state(false);
	let newName = $state('');
	let isDeleting = $state(false);
	let loaded = $state(false);

	let userPicture = $state($user?.picture);

	let deleteConfirmationModal: ReturnType<typeof Modal> | undefined = $state();

	const userSettings = inject(SETTINGS);
	const editorOptions: CodeEditorSettings[] = [
		{ schemeIdentifer: 'vscodium', displayName: 'VSCodium' },
		{ schemeIdentifer: 'vscode', displayName: 'VSCode' },
		{ schemeIdentifer: 'vscode-insiders', displayName: 'VSCode Insiders' },
		{ schemeIdentifer: 'windsurf', displayName: 'Windsurf' },
		{ schemeIdentifer: 'zed', displayName: 'Zed' },
		{ schemeIdentifer: 'cursor', displayName: 'Cursor' }
	];
	const editorOptionsForSelect = editorOptions.map((option) => ({
		label: option.displayName,
		value: option.schemeIdentifer
	}));

	$effect(() => {
		if ($user && !loaded) {
			loaded = true;
			userService.getUser().then((cloudUser) => {
				const userData: User = {
					...cloudUser,
					name: cloudUser.name || undefined,
					email: cloudUser.email || undefined,
					login: cloudUser.login || undefined,
					picture: cloudUser.picture || '#',
					locale: cloudUser.locale || 'en',
					access_token: cloudUser.access_token || 'impossible-situation',
					role: cloudUser.role || 'user',
					supporter: cloudUser.supporter || false,
					github_access_token: $user?.github_access_token,
					github_username: $user?.github_username
				};
				userPicture = userData.picture;
				userService.setUser(userData);
			});
			newName = $user?.name || '';
		}
	});

	let selectedPictureFile: File | undefined = $state();

	async function onSubmit(e: SubmitEvent) {
		if (!$user) return;
		saving = true;

		e.preventDefault();

		try {
			const updatedUser = await userService.updateUser({
				name: newName,
				picture: selectedPictureFile
			});
			updatedUser.github_access_token = $user?.github_access_token; // prevent overwriting with null
			userService.setUser(updatedUser);
			chipToasts.success('Profile updated');
			selectedPictureFile = undefined;
		} catch (err: any) {
			console.error(err);
			showError('Failed to update user', err);
		}
		saving = false;
	}

	function onPictureChange(file: File) {
		selectedPictureFile = file;
		userPicture = URL.createObjectURL(file);
	}

	async function onDeleteClicked() {
		isDeleting = true;
		try {
			await settingsService.deleteAllData();
			projectsService.unsetLastOpenedProject();
			await userService.logout();
			// TODO: Delete user from observable!!!
			chipToasts.success('All data deleted');
			goto('/', { replaceState: true, invalidateAll: true });
		} catch (err: any) {
			console.error(err);
			showError('Failed to delete project', err);
		} finally {
			deleteConfirmationModal?.close();
			isDeleting = false;
		}
	}

	let showSymlink = $state(false);
</script>

{#if $user}
	<SectionCard>
		<form onsubmit={onSubmit} class="profile-form">
			<ProfilePictureUpload
				bind:picture={userPicture}
				onFileSelect={onPictureChange}
				onInvalidFileType={() => chipToasts.error('Please use a valid image file')}
			/>

			<div id="contact-info" class="contact-info">
				<div class="contact-info__fields">
					<Textbox label="Full name" bind:value={newName} required />
					<Textbox label="Email" bind:value={$user.email} readonly />
				</div>

				<Button type="submit" style="pop" loading={saving}>Update profile</Button>
			</div>
		</form>
	</SectionCard>
{:else}
	<WelcomeSigninAction />
{/if}
<Spacer />

<SectionCard orientation="row" centerAlign>
	{#snippet title()}
		Default code editor
	{/snippet}
	{#snippet actions()}
		<Select
			value={$userSettings.defaultCodeEditor.schemeIdentifer}
			options={editorOptionsForSelect}
			onselect={(value) => {
				const selected = editorOptions.find((option) => option.schemeIdentifer === value);
				if (selected) {
					userSettings.update((s) => ({ ...s, defaultCodeEditor: selected }));
				}
			}}
		>
			{#snippet itemSnippet({ item, highlighted })}
				<SelectItem
					selected={item.value === $userSettings.defaultCodeEditor.schemeIdentifer}
					{highlighted}
				>
					{item.label}
				</SelectItem>
			{/snippet}
		</Select>
	{/snippet}
</SectionCard>

<SectionCard labelFor="disable-auto-checks" orientation="row">
	{#snippet title()}
		Automatically check for updates
	{/snippet}

	{#snippet caption()}
		Automatically check for updates. You can still check manually when needed.
	{/snippet}

	{#snippet actions()}
		<Toggle
			id="disable-auto-checks"
			checked={!$disableAutoChecks}
			onclick={() => ($disableAutoChecks = !$disableAutoChecks)}
		/>
	{/snippet}
</SectionCard>

<SectionCard orientation="column">
	{#snippet title()}
		Install the GitButler CLI (but)
	{/snippet}

	{#snippet caption()}
		Installs the GitButler CLI (but) in your PATH, allowing you to use it from the terminal. This
		action will request admin privileges. Alternatively, you could create a symlink manually.

		{#if showSymlink}
			<CliSymLink class="m-top-14" />
		{/if}
	{/snippet}

	<div class="flex flex-col gap-16">
		<div class="flex gap-8 justify-end">
			<Button
				style="pop"
				icon="play"
				onclick={async () => await instalCLI()}
				loading={installingCLI.current.isLoading}
			>
				Install But CLI</Button
			>
			<Button
				style="neutral"
				kind="outline"
				disabled={showSymlink}
				onclick={() => (showSymlink = !showSymlink)}>Show symlink</Button
			>
		</div>
	</div>
</SectionCard>

<Spacer />

{#if $user}
	<SectionCard orientation="row">
		{#snippet title()}
			Signing out
		{/snippet}
		{#snippet caption()}
			Ready to take a break? Click here to log out and unwind.
		{/snippet}

		<Login />
	</SectionCard>
{/if}

<SectionCard orientation="row">
	{#snippet title()}
		Remove all projects
	{/snippet}
	{#snippet caption()}
		You can delete all projects from the GitButler app.
		<br />
		Your code remains safe. it only clears the configuration.
	{/snippet}

	<Button style="error" kind="outline" onclick={() => deleteConfirmationModal?.show()}>
		Remove projects…
	</Button>

	<Modal
		bind:this={deleteConfirmationModal}
		width="small"
		title="Remove all projects"
		onSubmit={onDeleteClicked}
	>
		<p>Are you sure you want to remove all GitButler projects?</p>

		{#snippet controls(close)}
			<Button style="error" kind="outline" loading={isDeleting} type="submit">Remove</Button>
			<Button style="pop" onclick={close}>Cancel</Button>
		{/snippet}
	</Modal>
</SectionCard>

<style lang="postcss">
	.profile-form {
		display: flex;
		gap: 24px;
	}

	.contact-info {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: flex-end;
		gap: 20px;
	}

	.contact-info__fields {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 12px;
	}
</style>
