import { stories } from "../stories/stories.ts";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import { DrawerItem } from "./SidebarRow.tsx";
import { Tooltip } from "./Tooltip.tsx";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=${node}`;

const meta = stories({
	title: "Sidebar/DrawerItem",
	id: "components-draweritem",
	component: DrawerItem,
	parameters: { design: { type: "figma", url: core("5375-4379") }, width: 334 },
});

/** A drawer, closed: its name, how many it holds and its chevron. */
export const Default = meta.story({
	args: { name: "Hidden projects", count: 24, folded: true },
});

/** A ghost icon button at a drawer's end, with its tooltip. */
const ghost = (icon: "plus" | "mixer", label: string) => (
	<Tooltip key={label} content={label}>
		<Button variant="ghost" iconOnly aria-label={label}>
			<Icon name={icon} />
		</Button>
	</Tooltip>
);

/** Its buttons: Add project always shows, Sort and show only under the pointer. */
export const WithButtons = meta.story({
	args: {
		name: "Hidden projects",
		count: 24,
		folded: true,
		actions: ghost("plus", "Add project"),
		hoverActions: ghost("mixer", "Sort and show"),
	},
});
