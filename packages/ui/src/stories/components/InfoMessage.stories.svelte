<script module lang="ts">
	import InfoMessage from '$components/InfoMessage.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Components / InfoMessage',
		component: InfoMessage,
		tags: ['autodocs'],
		parameters: {
			docs: {
				description: {
					component:
						'The InfoMessage component displays informational, warning, error, or success messages with optional actions. It supports customizable icons, titles, content, and up to three action buttons.\n\n## Basic Usage\n\n```svelte\n<InfoMessage style="info">\n\t{#snippet title()}Important Information{/snippet}\n\t{#snippet content()}This is the message content.{/snippet}\n</InfoMessage>\n```'
				}
			}
		},
		args: {
			style: 'info',
			outlined: true,
			filled: false,
			shadow: false,
			primaryLabel: '',
			secondaryLabel: '',
			tertiaryLabel: ''
		},
		argTypes: {
			style: {
				description: 'The visual style and semantic meaning of the message',
				options: ['info', 'warning', 'error', 'success'],
				control: { type: 'select' }
			},
			outlined: {
				description: 'Whether to show a border around the message',
				control: { type: 'boolean' }
			},
			filled: {
				description: 'Whether to fill the background with a colored tint',
				control: { type: 'boolean' }
			},
			shadow: {
				description: 'Whether to show a drop shadow',
				control: { type: 'boolean' }
			},
			primaryLabel: {
				description: 'Label for the primary action button',
				control: { type: 'text' }
			},
			secondaryLabel: {
				description: 'Label for the secondary action button',
				control: { type: 'text' }
			},
			tertiaryLabel: {
				description: 'Label for the tertiary action button',
				control: { type: 'text' }
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
					'Interactive playground to test different InfoMessage configurations. Use the controls panel to modify properties and see the changes in real-time.'
			}
		}
	}}
>
	{#snippet template(args)}
		<div style="padding: 20px; background: var(--clr-bg-1);">
			<InfoMessage
				style={args.style}
				outlined={args.outlined}
				filled={args.filled}
				shadow={args.shadow}
				primaryLabel={args.primaryLabel}
				primaryAction={() => alert('Primary action clicked!')}
				secondaryLabel={args.secondaryLabel}
				secondaryAction={() => alert('Secondary action clicked!')}
				tertiaryLabel={args.tertiaryLabel}
				tertiaryAction={() => alert('Tertiary action clicked!')}
			>
				{#snippet title()}
					Message Title
				{/snippet}
				{#snippet content()}
					This is the message content. You can include longer explanatory text here.
				{/snippet}
			</InfoMessage>
		</div>
	{/snippet}
</Story>

<Story
	name="All Styles"
	parameters={{
		docs: {
			description: {
				story:
					'Showcases all available message styles: info (default information), warning (caution), error (negative feedback), and success (positive feedback). Each style has distinct visual styling and icons to convey the appropriate message tone.'
			}
		}
	}}
>
	{#snippet template()}
		<div
			style="padding: 20px; background: var(--clr-bg-1); display: flex; flex-direction: column; gap: 16px;"
		>
			<InfoMessage style="info">
				{#snippet title()}
					Information
				{/snippet}
				{#snippet content()}
					This is an informational message.
				{/snippet}
			</InfoMessage>

			<InfoMessage style="warning">
				{#snippet title()}
					Warning
				{/snippet}
				{#snippet content()}
					This is a warning message. Please pay attention.
				{/snippet}
			</InfoMessage>

			<InfoMessage style="danger">
				{#snippet title()}
					Error
				{/snippet}
				{#snippet content()}
					An error has occurred. Please review and try again.
				{/snippet}
			</InfoMessage>

			<InfoMessage style="success">
				{#snippet title()}
					Success
				{/snippet}
				{#snippet content()}
					Operation completed successfully!
				{/snippet}
			</InfoMessage>
		</div>
	{/snippet}
</Story>

<Story
	name="With Actions"
	parameters={{
		docs: {
			description: {
				story:
					'Examples showing InfoMessage with action buttons. You can have up to three action buttons (primary, secondary, and tertiary) with custom labels and icons.'
			}
		}
	}}
>
	{#snippet template()}
		<div
			style="padding: 20px; background: var(--clr-bg-1); display: flex; flex-direction: column; gap: 16px;"
		>
			<InfoMessage
				style="info"
				primaryLabel="Continue"
				primaryAction={() => alert('Continue clicked!')}
			>
				{#snippet title()}
					Action Required
				{/snippet}
				{#snippet content()}
					Please review the information and continue when ready.
				{/snippet}
			</InfoMessage>

			<InfoMessage
				style="warning"
				primaryLabel="Confirm"
				primaryAction={() => alert('Confirmed!')}
				secondaryLabel="Cancel"
				secondaryAction={() => alert('Cancelled!')}
			>
				{#snippet title()}
					Confirmation Needed
				{/snippet}
				{#snippet content()}
					Are you sure you want to proceed with this action?
				{/snippet}
			</InfoMessage>

			<InfoMessage
				style="danger"
				primaryLabel="Retry"
				primaryAction={() => alert('Retry clicked!')}
				secondaryLabel="Cancel"
				secondaryAction={() => alert('Cancelled!')}
				tertiaryLabel="Learn More"
				tertiaryAction={() => alert('Learn more clicked!')}
			>
				{#snippet title()}
					Operation Failed
				{/snippet}
				{#snippet content()}
					The operation could not be completed. You can retry or contact support.
				{/snippet}
			</InfoMessage>
		</div>
	{/snippet}
</Story>

<Story
	name="Filled Background"
	parameters={{
		docs: {
			description: {
				story:
					'Examples with filled backgrounds. The filled prop adds a subtle colored background tint matching the message style.'
			}
		}
	}}
>
	{#snippet template()}
		<div
			style="padding: 20px; background: var(--clr-bg-1); display: flex; flex-direction: column; gap: 16px;"
		>
			<InfoMessage style="info" filled>
				{#snippet title()}
					Filled Info Message
				{/snippet}
				{#snippet content()}
					This message has a filled background.
				{/snippet}
			</InfoMessage>

			<InfoMessage style="warning" filled>
				{#snippet title()}
					Filled Warning Message
				{/snippet}
				{#snippet content()}
					This warning has a filled background.
				{/snippet}
			</InfoMessage>

			<InfoMessage style="danger" filled>
				{#snippet title()}
					Filled Error Message
				{/snippet}
				{#snippet content()}
					This error has a filled background.
				{/snippet}
			</InfoMessage>

			<InfoMessage style="success" filled>
				{#snippet title()}
					Filled Success Message
				{/snippet}
				{#snippet content()}
					This success message has a filled background.
				{/snippet}
			</InfoMessage>
		</div>
	{/snippet}
</Story>

<Story
	name="With Error Block"
	parameters={{
		docs: {
			description: {
				story:
					'Example showing an error message with a technical error block. The error block displays in a code-style format with copy functionality.'
			}
		}
	}}
>
	{#snippet template()}
		<div style="padding: 20px; background: var(--clr-bg-1);">
			<InfoMessage
				primaryLabel="Retry"
				primaryAction={() => alert('Retry clicked!')}
				style="danger"
				error="Error: Failed to connect to server at https://api.example.com
Status: 500 Internal Server Error
Details: Connection timeout after 30 seconds"
			>
				{#snippet title()}
					Connection Error
				{/snippet}
				{#snippet content()}
					We couldn't connect to the server. Please check your internet connection and try again.
				{/snippet}
			</InfoMessage>
		</div>
	{/snippet}
</Story>

<Story
	name="Without Border"
	parameters={{
		docs: {
			description: {
				story:
					'Examples with the outlined prop set to false, removing the border for a cleaner look.'
			}
		}
	}}
>
	{#snippet template()}
		<div
			style="padding: 20px; background: var(--clr-bg-1); display: flex; flex-direction: column; gap: 16px;"
		>
			<InfoMessage style="info" outlined={false}>
				{#snippet title()}
					Borderless Info
				{/snippet}
				{#snippet content()}
					This message has no border.
				{/snippet}
			</InfoMessage>

			<InfoMessage style="success" outlined={false} filled>
				{#snippet title()}
					Borderless with Fill
				{/snippet}
				{#snippet content()}
					This message has no border but has a filled background.
				{/snippet}
			</InfoMessage>
		</div>
	{/snippet}
</Story>

<Story
	name="With Shadow"
	parameters={{
		docs: {
			description: {
				story: 'Example showing messages with drop shadows for elevated appearance.'
			}
		}
	}}
>
	{#snippet template()}
		<div
			style="padding: 20px; background: var(--clr-bg-1); display: flex; flex-direction: column; gap: 16px;"
		>
			<InfoMessage style="info" shadow>
				{#snippet title()}
					Message with Shadow
				{/snippet}
				{#snippet content()}
					This message has a drop shadow for visual elevation.
				{/snippet}
			</InfoMessage>
		</div>
	{/snippet}
</Story>

<Story
	name="Content Only"
	parameters={{
		docs: {
			description: {
				story: 'Example showing a minimal message with only content, no title.'
			}
		}
	}}
>
	{#snippet template()}
		<div style="padding: 20px; background: var(--clr-bg-1);">
			<InfoMessage style="info">
				{#snippet content()}
					This is a simple message without a title.
				{/snippet}
			</InfoMessage>
		</div>
	{/snippet}
</Story>
