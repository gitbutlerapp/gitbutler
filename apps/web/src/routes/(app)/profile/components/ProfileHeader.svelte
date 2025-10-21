<script lang="ts">
	import {
		ProfilePictureUpload,
		SectionCard,
		Button,
		Textbox,
		Toggle,
		Spacer
	} from '@gitbutler/ui';
	import type { User, UserService } from '$lib/user/userService';

	interface Props {
		user: User;
		userService: UserService;
	}

	let { user, userService }: Props = $props();

	let updatingName = $state(false);
	let updatingAdditionalInfo = $state(false);
	let nameValue = $state('');
	let emailValue = $state('');
	let websiteValue = $state('');
	let twitterValue = $state('');
	let blueskyValue = $state('');
	let locationValue = $state('');
	let emailShareValue = $state(false);

	$effect(() => {
		nameValue = user.name;
		emailValue = user.email;
		websiteValue = user.website || '';
		twitterValue = user.twitter || '';
		blueskyValue = user.bluesky || '';
		locationValue = user.location || '';
		emailShareValue = user.emailShare || false;
	});

	function onPictureChange(file: File) {
		updateProfilePicture(file);
	}

	async function updateProfilePicture(file: File) {
		try {
			await userService.updateUser({ picture: file });
		} catch (error) {
			console.error('Failed to update profile picture:', error);
			// TODO: Add toast notification for error
		}
	}

	async function updateName() {
		if (nameValue === user?.name) return;
		updatingName = true;
		try {
			await userService.updateUser({ name: nameValue });
		} finally {
			updatingName = false;
		}
	}

	async function updateAdditionalInfo() {
		updatingAdditionalInfo = true;
		try {
			await userService.updateUser({
				website: websiteValue,
				twitter: twitterValue,
				bluesky: blueskyValue,
				location: locationValue,
				timezone: '', // Not used in the UI currently
				emailShare: emailShareValue
			});
		} finally {
			updatingAdditionalInfo = false;
		}
	}
</script>

<form onsubmit={updateAdditionalInfo}>
	<SectionCard roundedBottom={false}>
		<div class="profile-header">
			<ProfilePictureUpload
				picture={user.picture}
				onFileSelect={onPictureChange}
				onInvalidFileType={() => {
					// TODO: Add toast notification for invalid file type
				}}
			/>

			<div class="contact-info__fields">
				<Textbox
					id="full-name"
					label="Full name"
					type="text"
					bind:value={nameValue}
					readonly={updatingName}
					onblur={updateName}
					onkeydown={(e) => e.key === 'Enter' && updateName()}
				/>
				<Textbox id="email" label="Email" type="text" bind:value={emailValue} readonly={true} />
			</div>
		</div>
	</SectionCard>

	<SectionCard roundedBottom={false} roundedTop={false}>
		<div class="contact-info__fields">
			<Textbox
				id="website"
				label="Website"
				type="url"
				placeholder="https://example.com"
				bind:value={websiteValue}
				readonly={updatingAdditionalInfo}
			/>

			<Textbox
				id="twitter"
				label="Twitter"
				type="text"
				placeholder="@username"
				bind:value={twitterValue}
				readonly={updatingAdditionalInfo}
			/>

			<Textbox
				id="bluesky"
				label="Bluesky"
				type="text"
				placeholder="@handle.bsky.social"
				bind:value={blueskyValue}
				readonly={updatingAdditionalInfo}
			/>

			<Textbox
				id="location"
				label="Location"
				type="text"
				placeholder="City, Country"
				bind:value={locationValue}
				readonly={updatingAdditionalInfo}
			/>

			<Spacer dotted />

			<label class="checkbox-section" for="email-share">
				<div class="checkbox-section__label">
					<h3 class="text-15 text-bold">Share my email</h3>
					<p class="text-12 text-body clr-text-2">Allow other users to see your email address.</p>
				</div>
				<Toggle
					id="email-share"
					checked={emailShareValue}
					disabled={updatingAdditionalInfo}
					onclick={() => (emailShareValue = !emailShareValue)}
				/>
			</label>
		</div>
	</SectionCard>

	<SectionCard roundedTop={false}>
		<div class="flex justify-end">
			<Button
				type="submit"
				style="pop"
				loading={updatingAdditionalInfo}
				disabled={updatingAdditionalInfo}
			>
				{updatingAdditionalInfo ? 'Saving...' : 'Update profile'}
			</Button>
		</div>
	</SectionCard>
</form>

<style lang="postcss">
	.profile-header {
		display: flex;
		gap: 24px;
	}

	.contact-info {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 20px;
	}

	.contact-info__fields {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 12px;
	}

	.notification-option {
		display: flex;
		align-items: flex-start;
	}

	.checkbox-label {
		display: flex;
		gap: 12px;
		cursor: pointer;
		user-select: none;
	}

	.checkbox-content {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.checkbox-title {
		color: var(--clr-scale-ntrl-0);
		font-weight: 500;
		font-size: 14px;
	}

	.checkbox-caption {
		color: var(--clr-scale-ntrl-30);
		font-size: 13px;
		line-height: 1.4;
	}

	.save-button {
		align-self: flex-end;
		margin-top: 16px;
		padding: 8px 16px;
		border: none;
		border-radius: var(--radius-m);
		background-color: var(--clr-scale-pop-70);
		color: var(--clr-core-ntrl-100);
		font-weight: 500;
		font-size: 14px;
		cursor: pointer;
		transition: background-color var(--transition-medium);

		&:hover:not(:disabled) {
			background-color: var(--clr-scale-pop-60);
		}

		&:disabled {
			cursor: not-allowed;
			opacity: 0.7;
		}
	}

	.checkbox-section {
		display: flex;
	}

	.checkbox-section__label {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 4px;
	}
</style>
