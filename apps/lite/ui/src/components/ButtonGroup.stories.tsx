import preview from "#storybook/preview";
import { getButtonClassName } from "#ui/components/Button.tsx";

import { ButtonGroup } from "./ButtonGroup.tsx";

const meta = preview.meta({
	component: ButtonGroup,
});

export const Default = meta.story({
	render: () => (
		<ButtonGroup aria-label="Default button group">
			<button type="button" className={getButtonClassName({ variant: "outline" })}>
				Left
			</button>
			<button type="button" className={getButtonClassName({ variant: "outline" })}>
				Middle
			</button>
			<button type="button" className={getButtonClassName({ variant: "outline" })}>
				Right
			</button>
		</ButtonGroup>
	),
});
