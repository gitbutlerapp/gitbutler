<script lang="ts" module>
	export interface Props {
		/**
		 * The URL of the current profile picture
		 */
		picture?: string;
		/**
		 * Alternative text for the image
		 */
		alt?: string;
		/**
		 * Array of accepted file types (e.g., ['image/jpeg', 'image/png'])
		 * @default ['image/jpeg', 'image/png']
		 */
		acceptedFileTypes?: string[];
		/**
		 * Callback when a valid file is selected
		 */
		onFileSelect?: (file: File) => void;
		/**
		 * Callback when an invalid file type is selected
		 */
		onInvalidFileType?: () => void;
		/**
		 * Custom class for the wrapper
		 */
		class?: string;
		/**
		 * Size of the profile picture
		 * @default 100
		 */
		size?: number;
	}
</script>

<script lang="ts">
	let {
		picture = $bindable(),
		alt = '',
		acceptedFileTypes = ['image/jpeg', 'image/png'],
		onFileSelect,
		onInvalidFileType,
		class: className,
		size = 6.25
	}: Props = $props();

	let previewUrl = $derived(picture);

	function handleFileChange(e: Event) {
		const target = e.target as HTMLInputElement;
		const file = target.files?.[0];

		if (file && acceptedFileTypes.includes(file.type)) {
			picture = URL.createObjectURL(file);
			onFileSelect?.(file);
		} else {
			onInvalidFileType?.();
		}
	}
</script>

<label
	class="profile-pic-wrapper focus-state {className || ''}"
	for="profile-picture-upload"
	style="width: {size}rem; height: {size}rem;"
>
	<input
		onchange={handleFileChange}
		type="file"
		id="profile-picture-upload"
		name="picture"
		accept={acceptedFileTypes.join(',')}
		class="hidden-input"
	/>

	{#if previewUrl}
		<img class="profile-pic" src={previewUrl} {alt} referrerpolicy="no-referrer" />
	{/if}

	<span class="profile-pic__edit-label text-11 text-semibold">Edit</span>
</label>

<style lang="postcss">
	.profile-pic-wrapper {
		display: flex;
		position: relative;
		flex-shrink: 0;
		overflow: hidden;
		border-radius: var(--radius-m);
		background-color: var(--clr-scale-pop-70);
		cursor: pointer;
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

	.hidden-input {
		z-index: var(--z-ground);
		position: absolute;
		width: 100%;
		height: 100%;
		cursor: pointer;
		opacity: 0;
	}

	.profile-pic {
		width: 100%;
		height: 100%;
		object-fit: cover;
		background-color: var(--clr-core-pop-70);
		transition: opacity var(--transition-medium);
	}

	.profile-pic__edit-label {
		position: absolute;
		bottom: 8px;
		left: 8px;
		padding: 4px 6px;
		border-radius: var(--radius-m);
		outline: 1px solid color-mix(in srgb, var(--clr-core-ntrl-100) 40%, transparent);
		background-color: var(--clr-core-ntrl-20);
		color: var(--clr-core-ntrl-100);
		opacity: 0;
		transition: opacity var(--transition-medium);
	}
</style>
