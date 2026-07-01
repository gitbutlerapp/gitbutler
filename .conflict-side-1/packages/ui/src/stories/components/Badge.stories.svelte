<script module lang="ts">
	import Badge from "$components/Badge.svelte";
	import { iconNames } from "$lib/icons/names";
	import { defineMeta } from "@storybook/addon-svelte-csf";

	const { Story } = defineMeta({
		title: "Basic / Badge",
		component: Badge as any,
		args: {
			text: "Badge",
			style: "gray",
			kind: "solid",
			size: "icon",
			icon: undefined,
		},
		argTypes: {
			text: {
				control: { type: "text" },
			},
			style: {
				options: ["gray", "pop", "safe", "warning", "danger", "purple"],
				control: { type: "select" },
			},
			kind: {
				options: ["solid", "soft"],
				control: { type: "select" },
			},
			size: {
				options: ["icon", "tag"],
				control: { type: "select" },
			},
			icon: {
				options: iconNames,
				control: { type: "select" },
			},
			reversedDirection: {
				control: { type: "boolean" },
			},
			skeleton: {
				control: { type: "boolean" },
			},
		},
	});
</script>

<Story name="default">
	{#snippet template(args)}
		<Badge
			style={args.style}
			kind={args.kind}
			icon={args.icon}
			size={args.size}
			reversedDirection={args.reversedDirection}
			skeleton={args.skeleton}
		>
			{args.text}
		</Badge>
	{/snippet}
</Story>

<Story name="Playground" />
