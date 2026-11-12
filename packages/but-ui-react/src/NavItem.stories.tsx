import { stories } from "../docs/stories.ts";
import { useState, type FC } from "react";
import { GroupLabel, NavItem } from "./SidebarRow.tsx";

const core = (node: string) =>
	`https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=${node}`;

const meta = stories({
	title: "Sidebar/NavItem",
	id: "components-navitem",
	component: NavItem,
	parameters: { design: { type: "figma", url: core("4793-4281") }, width: 316 },
});

/** A page in a navigation panel: its glyph and its name. */
export const Default = meta.story({
	args: { name: "Overview", icon: "docs" },
});

const DocsNavigation: FC = () => {
	const [folded, setFolded] = useState(false);
	const [groupFolded, setGroupFolded] = useState(false);
	return (
		// In a card, as every sidebar row is: never loose on a panel's ground. The leading row sits
		// alone past a hairline, and the group under it.
		<nav
			aria-label="Pages"
			style={{
				overflow: "hidden",
				border: "1px solid var(--but-border-subtle)",
				borderRadius: "var(--but-radius-card)",
				backgroundColor: "var(--but-background)",
			}}
		>
			<div style={{ padding: 8 }}>
				<NavItem
					leadingRow
					name="Docs"
					icon="docs"
					folded={folded}
					onToggleFolded={() => setFolded(!folded)}
					render={
						// oxlint-disable-next-line jsx_a11y/anchor-has-content, jsx_a11y/control-has-associated-label -- The row renders as this link, and NavItem fills it with the name.
						<a href="#docs" />
					}
				/>
			</div>
			{!folded && (
				<div
					style={{
						display: "flex",
						flexDirection: "column",
						gap: 1,
						padding: 8,
						borderTop: "1px solid var(--but-border-subtle)",
					}}
				>
					<GroupLabel
						name="Getting started"
						folded={groupFolded}
						onToggleFolded={() => setGroupFolded(!groupFolded)}
					/>
					{!groupFolded &&
						(
							[
								["Overview", "docs"],
								["Emphasis", "bullseye"],
								["Icons", "file-text"],
								["Buttons", "bullet-list"],
							] as const
						).map(([page, icon]) => (
							<NavItem
								key={page}
								name={page}
								icon={icon}
								depth={1}
								selected={page === "Emphasis"}
								render={
									// oxlint-disable-next-line jsx_a11y/anchor-has-content, jsx_a11y/control-has-associated-label -- The row renders as this link, and NavItem fills it with the name.
									<a
										href={`#${page.toLowerCase()}`}
										aria-current={page === "Emphasis" ? "page" : undefined}
									/>
								}
							/>
						))}
				</div>
			)}
		</nav>
	);
};

/** A docs site's card of pages: the leading row that opens the card's own page and folds it away,
 * a group label that folds its pages, and the pages one level under it, the page on screen
 * selected. */
export const Navigation = meta.story({
	render: () => <DocsNavigation />,
});

const AppearanceSettings: FC = () => {
	const [folded, setFolded] = useState(false);
	return (
		<nav
			aria-label="Settings"
			style={{
				overflow: "hidden",
				border: "1px solid var(--but-border-subtle)",
				borderRadius: "var(--but-radius-card)",
				backgroundColor: "var(--but-background)",
			}}
		>
			<div style={{ padding: 8 }}>
				<NavItem
					leadingRow
					name="Appearance"
					icon="contrast"
					folded={folded}
					onToggleFolded={() => setFolded(!folded)}
				/>
			</div>
			{!folded && (
				<div
					style={{
						display: "flex",
						flexDirection: "column",
						gap: 1,
						padding: 8,
						borderTop: "1px solid var(--but-border-subtle)",
					}}
				>
					<NavItem
						name="Radius"
						icon="corner-large"
						meta="Round"
						// oxlint-disable-next-line jsx_a11y/control-has-associated-label -- The row renders as this button, and NavItem fills it with the name.
						render={<button type="button" />}
					/>
					<NavItem
						name="Theme"
						icon="theme-light"
						meta="Light"
						// oxlint-disable-next-line jsx_a11y/control-has-associated-label -- The row renders as this button, and NavItem fills it with the name.
						render={<button type="button" />}
					/>
					<NavItem
						name="Color"
						icon={
							<span
								style={{
									width: 14,
									height: 14,
									borderRadius: "50%",
									backgroundColor: "var(--but-teal-8)",
								}}
							/>
						}
						meta="Teal"
						// oxlint-disable-next-line jsx_a11y/control-has-associated-label -- The row renders as this button, and NavItem fills it with the name.
						render={<button type="button" />}
					/>
				</div>
			)}
		</nav>
	);
};

/** A settings card: each row's `icon` is a glyph or an element of the host's own, such as the
 * chosen colour's swatch, and its `meta` is the current value, as plain text. */
export const Settings = meta.story({
	render: () => <AppearanceSettings />,
});
