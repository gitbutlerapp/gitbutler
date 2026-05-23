import preview from "#storybook/preview";
import { fn } from "storybook/test";

import { Button } from "./Button.tsx";
import { ButtonGroup } from "./ButtonGroup.tsx";

const meta = preview.meta({
	component: ButtonGroup,
});

export const Default = meta.story({
	render: () => (
		<ButtonGroup aria-label="Default button group">
			<Button variant="outline" onClick={fn()}>
				Left
			</Button>
			<Button variant="outline" onClick={fn()}>
				Middle
			</Button>
			<Button variant="outline" onClick={fn()}>
				Right
			</Button>
		</ButtonGroup>
	),
});
