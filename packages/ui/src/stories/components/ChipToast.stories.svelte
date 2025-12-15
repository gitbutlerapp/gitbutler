<script module lang="ts">
	import ChipToast from '$components/chipToast/ChipToast.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Overlays / ChipToast',
		component: ChipToast,
		tags: ['autodocs'],
		parameters: {
			docs: {
				description: {
					component:
						"The ChipToast component displays temporary messages to users with different types (info, success, warning, error) and supports custom action buttons and dismiss functionality. Use the toast system from @gitbutler/ui for programmatic toasts, or use this component directly for custom implementations.\n\n## Basic Usage\n\n```javascript\nimport { toasts, ToastContainer } from '@gitbutler/ui';\n\n// Show programmatic toasts\ntoasts.success('Operation completed!');\ntoasts.warning('Please review your changes');\ntoasts.error('Something went wrong');\n\n// Add container to your app root\n<ToastContainer />\n```\n\n**IMPORTANT**: Use only short messages in toasts, as they are designed for brief notifications. For longer messages, consider using regular toast messages or a modal instead."
				}
			}
		},
		args: {
			type: 'info',
			message: 'This is a toast message',
			showDismiss: false,
			customButton: undefined
		},
		argTypes: {
			type: {
				description: 'The visual style and semantic meaning of the toast',
				options: ['info', 'success', 'warning', 'error'],
				control: { type: 'select' }
			},
			message: {
				description: 'The text content to display in the toast',
				control: { type: 'text' }
			},
			showDismiss: {
				description: 'Whether to show the dismiss (X) button',
				control: { type: 'boolean' }
			},
			customButton: {
				description: 'Optional custom action button with label and action callback',
				control: { type: 'object' }
			}
		}
	});
</script>

<Story
	name="Playground"
	parameters={{
		docs: {
			description: {
				story:
					'Interactive playground to test different ChipToast configurations. Use the controls panel to modify properties and see the changes in real-time.'
			}
		}
	}}
>
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

<Story
	name="All Types"
	parameters={{
		docs: {
			description: {
				story:
					"Showcases all available toast types: info (default/info), success (positive feedback), warning (caution), and error (negative feedback). Each type has distinct visual styling to convey the appropriate message tone.\n\n**Usage:**\n```javascript\n// Different toast types\ntoasts.info('This is a info message');\ntoasts.success('Operation completed successfully!');\ntoasts.warning('Please review your changes');\ntoasts.error('Something went wrong');\n```"
			}
		}
	}}
>
	{#snippet template()}
		<div
			style="padding: 20px; background: var(--clr-bg-1); display: flex; flex-direction: column; gap: 16px;"
		>
			<ChipToast type="info" message="Info toast message" />
			<ChipToast type="success" message="Success! Operation completed successfully" />
			<ChipToast type="warning" message="Warning: Please check your settings" />
			<ChipToast type="danger" message="Error: Something went wrong" />
		</div>
	{/snippet}
</Story>

<Story
	name="With Custom and Dismiss Buttons"
	parameters={{
		docs: {
			description: {
				story:
					'Examples showing toasts with both custom action buttons and dismiss functionality. Users can interact with the toast through actions or dismiss it entirely.'
			}
		}
	}}
>
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
				type="danger"
				message="Failed to save changes"
				showDismiss={true}
				customButton={{
					label: 'Retry',
					action: () => alert('Retrying...')
				}}
				onDismiss={() => alert('Toast dismissed!')}
			/>

			<ChipToast
				type="info"
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

<Story
	name="Actions Only"
	parameters={{
		docs: {
			description: {
				story:
					'Examples showing toasts with either custom action buttons only or dismiss buttons only, demonstrating individual interaction patterns.'
			}
		}
	}}
>
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

			<ChipToast type="info" message="Information saved" showDismiss={true} />
		</div>
	{/snippet}
</Story>
