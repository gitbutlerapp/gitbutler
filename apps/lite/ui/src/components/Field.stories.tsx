import preview from "#storybook/preview";
import {
	FieldControlStyles,
	FieldControlWithIcon,
	FieldLabelStyles,
	FieldRootStyles,
	FieldTextareaStyles,
} from "#ui/components/Field.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { Field } from "@base-ui/react";

const meta = preview.meta({
	component: Field.Root,
});

export const Default = meta.story({
	render: () => (
		<Field.Root render={<FieldRootStyles />} style={{ width: 320 }}>
			<Field.Label render={<FieldLabelStyles />}>Label</Field.Label>
			<Field.Control
				render={<FieldControlStyles />}
				className="text-13"
				placeholder="Placeholder"
			/>
		</Field.Root>
	),
});

export const WithValue = meta.story({
	render: () => (
		<Field.Root render={<FieldRootStyles />} style={{ width: 320 }}>
			<Field.Label render={<FieldLabelStyles />}>Branch name</Field.Label>
			<Field.Control
				render={<FieldControlStyles />}
				className="text-13"
				defaultValue="feat/my-feature-branch"
			/>
		</Field.Root>
	),
});

export const Disabled = meta.story({
	render: () => (
		<Field.Root render={<FieldRootStyles />} style={{ width: 320 }}>
			<Field.Label render={<FieldLabelStyles />}>Read-only field</Field.Label>
			<Field.Control
				render={<FieldControlStyles />}
				className="text-13"
				defaultValue="Cannot be edited"
				disabled
			/>
		</Field.Root>
	),
});

export const Textarea = meta.story({
	render: () => (
		<Field.Root render={<FieldRootStyles />} style={{ width: 320 }}>
			<Field.Label render={<FieldLabelStyles />}>Description</Field.Label>
			<Field.Control
				render={<FieldTextareaStyles />}
				className="text-14 text-body"
				placeholder="Add a description…"
			/>
		</Field.Root>
	),
});

export const WithLeadingIcon = meta.story({
	render: () => (
		<Field.Root render={<FieldRootStyles />} style={{ width: 320 }}>
			<Field.Label render={<FieldLabelStyles />}>Search</Field.Label>
			<FieldControlWithIcon
				icon={<Icon name="search" size={16} />}
				iconPosition="leading"
				className="text-13"
				placeholder="Search…"
			/>
		</Field.Root>
	),
});

export const WithTrailingIcon = meta.story({
	render: () => (
		<Field.Root render={<FieldRootStyles />} style={{ width: 320 }}>
			<Field.Label render={<FieldLabelStyles />}>Repository URL</Field.Label>
			<FieldControlWithIcon
				icon={<Icon name="link" size={16} />}
				iconPosition="trailing"
				className="text-13"
				placeholder="https://github.com/org/repo"
			/>
		</Field.Root>
	),
});

export const AllVariants = meta.story({
	render: () => (
		<div style={{ display: "flex", flexDirection: "column", gap: 16, width: 320 }}>
			<Field.Root render={<FieldRootStyles />}>
				<Field.Label render={<FieldLabelStyles />}>Default</Field.Label>
				<Field.Control
					render={<FieldControlStyles />}
					className="text-13"
					placeholder="Placeholder"
				/>
			</Field.Root>

			<Field.Root render={<FieldRootStyles />}>
				<Field.Label render={<FieldLabelStyles />}>With value</Field.Label>
				<Field.Control
					render={<FieldControlStyles />}
					className="text-13"
					defaultValue="Some input value"
				/>
			</Field.Root>

			<Field.Root render={<FieldRootStyles />}>
				<Field.Label render={<FieldLabelStyles />}>Disabled</Field.Label>
				<Field.Control
					render={<FieldControlStyles />}
					className="text-13"
					defaultValue="Disabled value"
					disabled
				/>
			</Field.Root>

			<Field.Root render={<FieldRootStyles />}>
				<Field.Label render={<FieldLabelStyles />}>Textarea</Field.Label>
				<Field.Control
					render={<FieldTextareaStyles />}
					className="text-14 text-body"
					placeholder="Multi-line input…"
				/>
			</Field.Root>
		</div>
	),
});
