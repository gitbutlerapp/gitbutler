import preview from "#storybook/preview";
import { FieldTextareaStyles } from "#ui/components/Field.tsx";
import { MarkdownToolbar } from "#ui/components/MarkdownToolbar.tsx";
import { Tooltip } from "@base-ui/react";
import { useRef, useState } from "react";
import type { FC } from "react";

const meta = preview.meta({
	component: MarkdownToolbar,
	decorators: [
		(Story) => (
			<Tooltip.Provider>
				<Story />
			</Tooltip.Provider>
		),
	],
});

const sample = `Move the commit toolbox above the lane

Selecting a commit used to hide the toolbox behind the graph.`;

/**
 * The toolbar only rewrites the textarea it is pointed at, so the demo owns
 * the source and hands the toolbar a ref to the field.
 */
const Demo: FC<{ disabled?: boolean; width?: number }> = ({ disabled, width = 480 }) => {
	const [value, setValue] = useState(sample);
	const bodyRef = useRef<HTMLTextAreaElement>(null);

	return (
		<div style={{ display: "flex", flexDirection: "column", gap: 8, width }}>
			<MarkdownToolbar disabled={disabled} onInput={setValue} targetRef={bodyRef} />
			<FieldTextareaStyles
				aria-label="Description"
				className="text-body"
				disabled={disabled}
				onChange={(evt) => setValue(evt.currentTarget.value)}
				placeholder="Description"
				ref={bodyRef}
				rows={6}
				value={value}
			/>
		</div>
	);
};

/** Select some text in the field, or place the caret, and press a button. */
export const Default = meta.story({
	render: () => <Demo />,
});

/**
 * Too narrow for every group, the buttons scroll a group at a time behind the
 * chevrons; a horizontal wheel or a swipe snaps to the same stops.
 */
export const Narrow = meta.story({
	render: () => <Demo width={300} />,
});

/** Disabled while a submit is pending: the buttons stay put but do nothing. */
export const Disabled = meta.story({
	render: () => <Demo disabled />,
});
