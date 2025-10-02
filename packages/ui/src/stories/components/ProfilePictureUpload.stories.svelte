<script module lang="ts">
	import ProfilePictureUpload from '$components/ProfilePictureUpload.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / ProfilePictureUpload',
		component: ProfilePictureUpload,
		args: {
			picture: 'https://avatars.githubusercontent.com/u/1942963?v=4',
			alt: 'User avatar',
			size: 100
		},
		argTypes: {
			size: {
				control: { type: 'number', min: 50, max: 200, step: 10 }
			}
		}
	});
</script>

<script lang="ts">
	import { chipToasts } from '$components/chipToast/chipToastStore';
</script>

<Story name="Default">
	{#snippet template(args)}
		<ProfilePictureUpload
			picture={args.picture}
			alt={args.alt}
			size={args.size}
			onFileSelect={(file) => {
				chipToasts.success(`File selected: ${file.name}`);
			}}
			onInvalidFileType={() => {
				chipToasts.error('Please use a valid image file (JPEG or PNG)');
			}}
		/>
	{/snippet}
</Story>

<Story name="Without Picture">
	{#snippet template()}
		<ProfilePictureUpload
			onFileSelect={(file) => {
				chipToasts.success(`File selected: ${file.name}`);
			}}
			onInvalidFileType={() => {
				chipToasts.error('Please use a valid image file (JPEG or PNG)');
			}}
		/>
	{/snippet}
</Story>

<Story name="Different Sizes">
	{#snippet template()}
		<div style="display: flex; gap: 24px; align-items: flex-end;">
			<div>
				<p style="margin-bottom: 8px; font-size: 12px; color: var(--clr-scale-ntrl-30);">
					Small (60px)
				</p>
				<ProfilePictureUpload
					picture="https://avatars.githubusercontent.com/u/1942963?v=4"
					alt="Small avatar"
					size={60}
					onFileSelect={(file) => {
						chipToasts.success(`File selected: ${file.name}`);
					}}
					onInvalidFileType={() => {
						chipToasts.error('Please use a valid image file');
					}}
				/>
			</div>
			<div>
				<p style="margin-bottom: 8px; font-size: 12px; color: var(--clr-scale-ntrl-30);">
					Default (100px)
				</p>
				<ProfilePictureUpload
					picture="https://avatars.githubusercontent.com/u/1942963?v=4"
					alt="Default avatar"
					size={100}
					onFileSelect={(file) => {
						chipToasts.success(`File selected: ${file.name}`);
					}}
					onInvalidFileType={() => {
						chipToasts.error('Please use a valid image file');
					}}
				/>
			</div>
			<div>
				<p style="margin-bottom: 8px; font-size: 12px; color: var(--clr-scale-ntrl-30);">
					Large (150px)
				</p>
				<ProfilePictureUpload
					picture="https://avatars.githubusercontent.com/u/1942963?v=4"
					alt="Large avatar"
					size={150}
					onFileSelect={(file) => {
						chipToasts.success(`File selected: ${file.name}`);
					}}
					onInvalidFileType={() => {
						chipToasts.error('Please use a valid image file');
					}}
				/>
			</div>
		</div>
	{/snippet}
</Story>

<Story name="Playground" />
