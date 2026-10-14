import preview from "#storybook/preview";
import { CopyableId } from "./CopyableId.tsx";

const meta = preview.meta({
	title: "Actions/CopyableId",
	id: "components-copyableid",
	component: CopyableId,
});

const copyToClipboard = (value: string) => void navigator.clipboard.writeText(value);

export const Default = meta.story({
	args: {
		label: "Copy commit ID",
		icon: "hash",
		value: "1cd28ec4f0a9b3e1d2c7a6f5e4b3a2918d7c6b5a",
		display: "1cd28ec",
		onCopy: copyToClipboard,
	},
});

/** A commit's two IDs side by side, as its author line shows them. */
export const ChangeAndCommit = meta.story({
	render: () => (
		<div className="text-13" style={{ display: "flex", gap: 8 }}>
			<CopyableId
				label="Copy change ID"
				icon="finger-print"
				value="kzqtpwoylnsrkrkmxqzvwtuoyrpslmnk"
				display="kzqtpwo"
				onCopy={copyToClipboard}
			/>
			<CopyableId
				label="Copy commit ID"
				icon="hash"
				value="1cd28ec4f0a9b3e1d2c7a6f5e4b3a2918d7c6b5a"
				display="1cd28ec"
				onCopy={copyToClipboard}
			/>
		</div>
	),
});
