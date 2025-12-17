<script lang="ts">
	import { goto } from '$app/navigation';
	import WelcomeSigninAction from '$components/WelcomeSigninAction.svelte';
	import CliSymLink from '$components/profileSettings/CliSymLink.svelte';
	import { BACKEND } from '$lib/backend';
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
		CardGroup,
		Modal,
		ProfilePictureUpload,
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

	const backend = inject(BACKEND);
	const platformName = backend.platformName;

	const appSettings = settingsService.appSettings;

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
		{ schemeIdentifer: 'cursor', displayName: 'Cursor' },
		{ schemeIdentifer: 'trae', displayName: 'Trae' }
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
					supporter: cloudUser.supporter || false
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
	<CardGroup>
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
	</CardGroup>

	<CardGroup>
		<CardGroup.Item>
			{#snippet title()}
				Signing out
			{/snippet}
			{#snippet caption()}
				Ready to take a break? Click here to log out and unwind.
			{/snippet}
			{#snippet actions()}
				<Button
					kind="outline"
					icon="signout"
					onclick={async () => {
						await userService.logout();
					}}>Log out</Button
				>
			{/snippet}
		</CardGroup.Item>
	</CardGroup>
{/if}

<WelcomeSigninAction />

<Spacer />

<CardGroup>
	<CardGroup.Item alignment="center">
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
	</CardGroup.Item>
</CardGroup>

<CardGroup>
	<CardGroup.Item labelFor="disable-auto-checks">
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
	</CardGroup.Item>
</CardGroup>

<CardGroup>
	<CardGroup.Item>
		{#snippet title()}
			Install the GitButler CLI <code class="code-string">but</code>
		{/snippet}

		{#snippet caption()}
			{#if $appSettings?.ui.cliIsManagedByPackageManager}
				The <code>but</code> CLI is managed by your package manager. Please use your package manager to
				install, update, or remove it.
			{:else if platformName === 'windows'}
				On Windows, you can manually copy the executable (<code>`but`</code>) to a directory in your
				PATH. Click "Show Command" for instructions.
			{:else}
				Installs the GitButler CLI (<code>`but`</code>) in your PATH, allowing you to use it from
				the terminal. This action will request admin privileges. Alternatively, you could create a
				symlink manually.
			{/if}
		{/snippet}

		{#if !$appSettings?.ui.cliIsManagedByPackageManager}
			<div class="flex flex-col gap-16">
				<div class="flex gap-8 justify-end">
					{#if platformName !== 'windows'}
						<Button
							style="pop"
							icon="play"
							onclick={async () => await instalCLI()}
							loading={installingCLI.current.isLoading}
						>
							Install But CLI</Button
						>
					{/if}
					<Button
						style="gray"
						kind="outline"
						disabled={showSymlink}
						onclick={() => (showSymlink = !showSymlink)}>Show command</Button
					>
				</div>
			</div>

			{#if showSymlink}
				<CliSymLink class="m-t-14" />
			{/if}
		{/if}
	</CardGroup.Item>
</CardGroup>

<Spacer />

<CardGroup>
	<CardGroup.Item>
		{#snippet title()}
			Remove all projects
		{/snippet}
		{#snippet caption()}
			You can delete all projects from the GitButler app.
			<br />
			Your code remains safe. it only clears the configuration.
		{/snippet}

		{#snippet actions()}
			<Button style="danger" kind="outline" onclick={() => deleteConfirmationModal?.show()}>
				Remove projects…
			</Button>
		{/snippet}
	</CardGroup.Item>
</CardGroup>

<Modal
	bind:this={deleteConfirmationModal}
	width="small"
	title="Remove all projects"
	onSubmit={onDeleteClicked}
>
	<p>Are you sure you want to remove all GitButler projects?</p>

	{#snippet controls(close)}
		<Button style="danger" kind="outline" loading={isDeleting} type="submit">Remove</Button>
		<Button style="pop" onclick={close}>Cancel</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.profile-form {
		display: flex;
		padding: 16px;
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
