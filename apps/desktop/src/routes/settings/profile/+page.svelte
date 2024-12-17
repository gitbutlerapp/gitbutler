<script lang="ts">
	import { deleteAllData } from '$lib/backend/data';
	import Login from '$lib/components/Login.svelte';
	import WelcomeSigninAction from '$lib/components/WelcomeSigninAction.svelte';
	import SettingsPage from '$lib/layout/SettingsPage.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { UserService } from '$lib/stores/user';
	import * as toasts from '$lib/utils/toasts';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';

	const userService = getContext(UserService);
	const user = userService.user;

	const fileTypes = ['image/jpeg', 'image/png'];

	let saving = $state(false);
	let newName = $state('');
	let isDeleting = $state(false);
	let loaded = $state(false);

	let userPicture = $state($user?.picture);

	let deleteConfirmationModal: ReturnType<typeof Modal> | undefined = $state();

	onMount(() => {
		if ($user && !loaded) {
			loaded = true;
			userService.getUser().then((cloudUser) => {
				cloudUser.github_access_token = $user?.github_access_token; // prevent overwriting with null
				userService.setUser(cloudUser);
			});
			newName = $user?.name || '';
		}
	});

	async function onSubmit(e: SubmitEvent) {
		if (!$user) return;
		saving = true;

		const target = e.target as HTMLFormElement;
		const formData = new FormData(target);
		const picture = formData.get('picture') as File | undefined;

		try {
			const updatedUser = await userService.updateUser({
				name: newName,
				picture: picture
			});
			updatedUser.github_access_token = $user?.github_access_token; // prevent overwriting with null
			userService.setUser(updatedUser);
			toasts.success('Profile updated');
		} catch (err: any) {
			console.error(err);
			showError('Failed to update user', err);
		}
		saving = false;
	}

	function onPictureChange(e: Event) {
		const target = e.target as HTMLInputElement;
		const file = target.files?.[0];

		if (file && fileTypes.includes(file.type)) {
			userPicture = URL.createObjectURL(file);
		} else {
			userPicture = $user?.picture;
			toasts.error('Please use a valid image file');
		}
	}

	async function onDeleteClicked() {
		isDeleting = true;
		try {
			deleteAllData();
			await userService.logout();
			// TODO: Delete user from observable!!!
			toasts.success('All data deleted');
			goto('/', { replaceState: true, invalidateAll: true });
		} catch (err: any) {
			console.error(err);
			showError('Failed to delete project', err);
		} finally {
			deleteConfirmationModal?.close();
			isDeleting = false;
		}
	}
</script>

<SettingsPage title="Profile">
	{#if $user}
		<SectionCard>
			<form onsubmit={onSubmit} class="profile-form">
				<label id="profile-picture" class="profile-pic-wrapper focus-state" for="picture">
					<input
						onchange={onPictureChange}
						type="file"
						id="picture"
						name="picture"
						accept={fileTypes.join(',')}
						class="hidden-input"
					/>

					{#if $user.picture}
						<img class="profile-pic" src={userPicture} alt="" referrerpolicy="no-referrer" />
					{/if}

					<span class="profile-pic__edit-label text-11 text-semibold">Edit</span>
				</label>

				<div id="contact-info" class="contact-info">
					<div class="contact-info__fields">
						<Textbox label="Full name" bind:value={newName} required />
						<Textbox label="Email" bind:value={$user.email} readonly />
					</div>

					<Button type="submit" style="pop" kind="solid" loading={saving}>Update profile</Button>
				</div>
			</form>
		</SectionCard>
	{:else}
		<WelcomeSigninAction />
	{/if}
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

		<Button style="error" kind="soft" onclick={() => deleteConfirmationModal?.show()}>
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
				<Button style="error" kind="solid" loading={isDeleting} type="submit">Remove</Button>
				<Button style="pop" kind="solid" onclick={close}>Cancel</Button>
			{/snippet}
		</Modal>
	</SectionCard>
</SettingsPage>

<style lang="postcss">
	.profile-form {
		display: flex;
		gap: 24px;
	}

	.hidden-input {
		cursor: pointer;
		z-index: var(--z-ground);
		position: absolute;
		width: 100%;
		height: 100%;
		opacity: 0;
	}

	.profile-pic-wrapper {
		position: relative;
		width: 100px;
		height: 100px;
		border-radius: var(--radius-m);
		overflow: hidden;
		background-color: var(--clr-scale-pop-70);
		transition: opacity var(--transition-medium);

		&:hover,
		&:focus-within {
			& .profile-pic__edit-label {
				opacity: 1;
			}

			& .profile-pic {
				opacity: 0.8;
			}
		}
	}

	.profile-pic {
		width: 100%;
		height: 100%;

		object-fit: cover;
		background-color: var(--clr-scale-pop-70);
	}

	.profile-pic__edit-label {
		position: absolute;
		bottom: 8px;
		left: 8px;
		color: var(--clr-core-ntrl-100);
		background-color: var(--clr-scale-ntrl-20);
		padding: 4px 6px;
		border-radius: var(--radius-m);
		opacity: 0;
		transition: opacity var(--transition-medium);
	}

	.contact-info {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 20px;
		align-items: flex-end;
	}

	.contact-info__fields {
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
</style>
