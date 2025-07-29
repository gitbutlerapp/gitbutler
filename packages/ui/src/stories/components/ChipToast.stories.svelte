<script module lang="ts">
	import ChipToast from '$components/chipToast/ChipToast.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Overlays / ChipToast',
		component: ChipToast,
		args: {
			type: 'neutral',
			message: 'This is a toast message',
			showDismiss: false,
			customButton: undefined
		},
		argTypes: {
			type: {
				options: ['neutral', 'success', 'warning', 'error'],
				control: { type: 'select' }
			},
			message: {
				control: { type: 'text' }
			},
			showDismiss: {
				control: { type: 'boolean' }
			},
			customButton: {
				control: { type: 'object' }
			}
		}
	});
</script>

<Story name="Playground">
	{#snippet template(args)}
		<div style="padding: 20px; background: var(--clr-bg-1);">
			<ChipToast
				type={args.type}
				message={args.message}
				showDismiss={args.showDismiss}
				customButton={args.customButton}
				onDismiss={() => alert('Toast dismissed!')}
			/>
		</div>
	{/snippet}
</Story>

<Story name="All Types">
	{#snippet template()}
		<div
			style="padding: 20px; background: var(--clr-bg-1); display: flex; flex-direction: column; gap: 16px;"
		>
			<ChipToast type="neutral" message="Neutral toast message" />
			<ChipToast type="success" message="Success! Operation completed successfully" />
			<ChipToast type="warning" message="Warning: Please check your settings" />
			<ChipToast type="error" message="Error: Something went wrong" />
		</div>
	{/snippet}
</Story>

<Story name="With Custom and Dismiss Buttons">
	{#snippet template()}
		<div
			style="padding: 20px; background: var(--clr-bg-1); display: flex; flex-direction: column; gap: 16px;"
		>
			<ChipToast
				type="success"
				message="File uploaded successfully!"
				showDismiss={true}
				customButton={{
					label: 'View File',
					action: () => alert('Opening file...')
				}}
				onDismiss={() => alert('Toast dismissed!')}
			/>

			<ChipToast
				type="warning"
				message="Your session will expire soon"
				showDismiss={true}
				customButton={{
					label: 'Extend Session',
					action: () => alert('Session extended!')
				}}
				onDismiss={() => alert('Toast dismissed!')}
			/>

			<ChipToast
				type="error"
				message="Failed to save changes"
				showDismiss={true}
				customButton={{
					label: 'Retry',
					action: () => alert('Retrying...')
				}}
				onDismiss={() => alert('Toast dismissed!')}
			/>

			<ChipToast
				type="neutral"
				message="New update available"
				showDismiss={true}
				customButton={{
					label: 'Update Now',
					action: () => alert('Updating...')
				}}
				onDismiss={() => alert('Toast dismissed!')}
			/>
		</div>
	{/snippet}
</Story>

<Story name="Actions Only">
	{#snippet template()}
		<div
			style="padding: 20px; background: var(--clr-bg-1); display: flex; flex-direction: column; gap: 16px;"
		>
			<ChipToast
				type="success"
				message="Operation completed"
				customButton={{
					label: 'View Details',
					action: () => alert('Showing details...')
				}}
			/>

			<ChipToast type="neutral" message="Information saved" showDismiss={true} />
		</div>
	{/snippet}
</Story>
