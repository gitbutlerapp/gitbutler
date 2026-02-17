<script module lang="ts">
	import Button from "$components/Button.svelte";
	import ChipToastContainer from "$components/chipToast/ChipToastContainer.svelte";
	import { chipToasts } from "$components/chipToast/chipToastStore";
	import { defineMeta } from "@storybook/addon-svelte-csf";

	const { Story } = defineMeta({
		title: "Overlays / ChipToast",
		component: ChipToastContainer,
		tags: ["autodocs"],
		parameters: {
			docs: {
				description: {
					component:
						"The ChipToast component displays temporary messages to users with different types (info, success, warning, error) and supports custom action buttons and dismiss functionality. Use the toast system from @gitbutler/ui for programmatic toasts.\n\n## Basic Usage\n\n```javascript\nimport { chipToasts, ChipToastContainer } from '@gitbutler/ui';\n\n// Show programmatic toasts\nchipToasts.success('Operation completed!');\nchipToasts.warning('Please review your changes');\nchipToasts.error('Something went wrong');\nchipToasts.info('Information message');\n\n// Add container to your app root\n<ChipToastContainer />\n```\n\n**IMPORTANT**: Use only short messages in toasts, as they are designed for brief notifications. For longer messages, consider using regular toast messages or a modal instead.",
				},
			},
		},
	});
</script>

<Story
	name="Playground"
	parameters={{
		docs: {
			description: {
				story:
					"Interactive playground to test the ChipToast system. Click the buttons below to trigger different types of toasts. Toasts will auto-dismiss after 4 seconds unless they have a dismiss button.",
			},
		},
	}}
>
	{#snippet template()}
		<div style="padding: 20px; background: var(--clr-bg-1);">
			<div style="display: flex; gap: 12px; flex-wrap: wrap; margin-bottom: 20px;">
				<Button size="tag" onclick={() => chipToasts.info("This is an info message")}>
					Show Info
				</Button>
				<Button size="tag" onclick={() => chipToasts.success("Operation completed successfully!")}>
					Show Success
				</Button>
				<Button
					size="tag"
					onclick={() => chipToasts.warning("Warning: Please check your settings")}
				>
					Show Warning
				</Button>
				<Button size="tag" onclick={() => chipToasts.error("Error: Something went wrong")}>
					Show Error
				</Button>
				<Button
					size="tag"
					onclick={() => chipToasts.info("Message with dismiss button", { showDismiss: true })}
				>
					With Dismiss
				</Button>
				<Button
					size="tag"
					onclick={() =>
						chipToasts.success("File uploaded!", {
							customButton: { label: "View", action: () => alert("View clicked!") },
						})}
				>
					With Action
				</Button>
				<Button size="tag" onclick={() => chipToasts.clearAll()}>Clear All</Button>
			</div>
			<ChipToastContainer />
		</div>
	{/snippet}
</Story>

<Story
	name="All Types"
	parameters={{
		docs: {
			description: {
				story:
					"Showcases all available toast types: info (default/info), success (positive feedback), warning (caution), and error (negative feedback). Each type has distinct visual styling to convey the appropriate message tone.\n\n**Usage:**\n```javascript\n// Different toast types\nchipToasts.info('This is an info message');\nchipToasts.success('Operation completed successfully!');\nchipToasts.warning('Please review your changes');\nchipToasts.error('Something went wrong');\n```",
			},
		},
	}}
>
	{#snippet template()}
		<div style="padding: 20px; background: var(--clr-bg-1);">
			<Button
				size="tag"
				onclick={() => {
					chipToasts.info("Info toast message");
					chipToasts.success("Success! Operation completed successfully");
					chipToasts.warning("Warning: Please check your settings");
					chipToasts.error("Error: Something went wrong");
				}}
			>
				Show All Types
			</Button>
			<ChipToastContainer />
		</div>
	{/snippet}
</Story>

<Story
	name="With Custom and Dismiss Buttons"
	parameters={{
		docs: {
			description: {
				story:
					"Examples showing toasts with both custom action buttons and dismiss functionality. Users can interact with the toast through actions or dismiss it entirely. Note that toasts with dismiss buttons will not auto-dismiss.",
			},
		},
	}}
>
	{#snippet template()}
		<div style="padding: 20px; background: var(--clr-bg-1);">
			<div style="display: flex; gap: 12px; flex-wrap: wrap; margin-bottom: 20px;">
				<Button
					size="tag"
					onclick={() =>
						chipToasts.success("File uploaded successfully!", {
							showDismiss: true,
							customButton: {
								label: "View File",
								action: () => alert("Opening file..."),
							},
						})}
				>
					Upload Success
				</Button>

				<Button
					size="tag"
					onclick={() =>
						chipToasts.warning("Your session will expire soon", {
							showDismiss: true,
							customButton: {
								label: "Extend",
								action: () => alert("Session extended!"),
							},
						})}
				>
					Session Warning
				</Button>

				<Button
					size="tag"
					onclick={() =>
						chipToasts.error("Failed to save changes", {
							showDismiss: true,
							customButton: {
								label: "Retry",
								action: () => alert("Retrying..."),
							},
						})}
				>
					Save Error
				</Button>

				<Button
					size="tag"
					onclick={() =>
						chipToasts.info("New update available", {
							showDismiss: true,
							customButton: {
								label: "Update",
								action: () => alert("Updating..."),
							},
						})}
				>
					Update Info
				</Button>
			</div>
			<ChipToastContainer />
		</div>
	{/snippet}
</Story>

<Story
	name="Actions Only"
	parameters={{
		docs: {
			description: {
				story:
					"Examples showing toasts with either custom action buttons only or dismiss buttons only, demonstrating individual interaction patterns.",
			},
		},
	}}
>
	{#snippet template()}
		<div style="padding: 20px; background: var(--clr-bg-1);">
			<div style="display: flex; gap: 12px; flex-wrap: wrap; margin-bottom: 20px;">
				<Button
					size="tag"
					onclick={() =>
						chipToasts.success("Operation completed", {
							customButton: {
								label: "View Details",
								action: () => alert("Showing details..."),
							},
						})}
				>
					With Action Button
				</Button>

				<Button
					size="tag"
					onclick={() => chipToasts.info("Information saved", { showDismiss: true })}
				>
					With Dismiss Button
				</Button>
			</div>
			<ChipToastContainer />
		</div>
	{/snippet}
</Story>
