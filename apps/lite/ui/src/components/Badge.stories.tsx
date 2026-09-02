import preview from "#storybook/preview";
import { Badge, type BadgeSize, type BadgeVariant } from "./Badge.tsx";
import { Icon } from "./Icon.tsx";

const meta = preview.meta({
	component: Badge,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=706-453",
		},
	},
	argTypes: {
		variant: {
			control: "select",
			options: [
				"fillGray",
				"lightGray",
				"safe",
				"warn",
				"danger",
				"purple",
				"blue",
			] satisfies Array<BadgeVariant>,
		},
		size: {
			control: "inline-radio",
			options: ["regular", "large"] satisfies Array<BadgeSize>,
		},
	},
	args: {
		children: "42",
		variant: "fillGray",
	},
});

export const Default = meta.story({ args: { variant: "fillGray", children: "42" } });

export const AllVariants = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 8, alignItems: "center" }}>
			<Badge variant="fillGray">fillGray</Badge>
			<Badge variant="lightGray">lightGray</Badge>
			<Badge variant="safe">safe</Badge>
			<Badge variant="warn">warn</Badge>
			<Badge variant="danger">danger</Badge>
			<Badge variant="purple">purple</Badge>
			<Badge variant="blue">blue</Badge>
		</div>
	),
});

export const AllSizes = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 8, alignItems: "center" }}>
			<Badge variant="fillGray">regular</Badge>
			<Badge variant="fillGray" size="large">
				large
			</Badge>
			<Badge variant="safe">42</Badge>
			<Badge variant="safe" size="large">
				42
			</Badge>
		</div>
	),
});

export const Inverted = meta.story({
	render: () => (
		<div
			style={{
				["--selection-inverted" as string]: "true",
				display: "flex",
				gap: 8,
				alignItems: "center",
				padding: 8,
				borderRadius: 6,
				backgroundColor: "var(--fill-gray-bg)",
				color: "var(--fill-gray-fg)",
			}}
		>
			<Badge variant="fillGray">fillGray</Badge>
			<Badge variant="lightGray">lightGray</Badge>
			<Badge variant="safe">safe</Badge>
			<Badge variant="warn">warn</Badge>
			<Badge variant="danger">danger</Badge>
		</div>
	),
});

export const WithIcon = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 8, alignItems: "center" }}>
			<Badge variant="safe">
				<Icon name="tick" size={12} />
			</Badge>
			<Badge variant="danger">
				<Icon name="cross" size={12} />
			</Badge>
			<Badge variant="lightGray">
				<Icon name="spinner" size={12} />
			</Badge>
			<Badge variant="warn">
				<Icon name="warning" size={12} />
			</Badge>
			<Badge variant="safe">
				<Icon name="tick" size={12} />
				Passed
			</Badge>
			<Badge variant="safe" size="large">
				<Icon name="tick" size={12} />
				Passed
			</Badge>
		</div>
	),
});

export const CIChecks = meta.story({
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
			{(
				[
					{ label: "success", variant: "safe", icon: "tick" },
					{ label: "failure", variant: "danger", icon: "cross" },
					{ label: "in progress", variant: "lightGray", icon: "spinner" },
					{ label: "in progress (some failed)", variant: "danger", icon: "spinner" },
					{ label: "cancelled", variant: "lightGray", icon: "cross" },
					{ label: "action required", variant: "warn", icon: "warning" },
					{ label: "unknown", variant: "lightGray", icon: "question" },
				] satisfies ReadonlyArray<{
					label: string;
					variant: BadgeVariant;
					icon: Parameters<typeof Icon>[0]["name"];
				}>
			).map(({ label, variant, icon }) => (
				<div key={label} style={{ display: "flex", gap: 8, alignItems: "center" }}>
					<Badge variant={variant}>
						<Icon name={icon} size={12} />
					</Badge>
					<span style={{ fontSize: 12 }}>{label}</span>
				</div>
			))}
		</div>
	),
});
