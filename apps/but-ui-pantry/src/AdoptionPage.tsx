import { Code } from "@gitbutler/but-ui-react/Code.tsx";
import { DescriptionList, DescriptionListItem } from "@gitbutler/but-ui-react/DescriptionList.tsx";
import { Table, TableCell, TableRow } from "@gitbutler/but-ui-react/Table.tsx";
import { TextLink } from "@gitbutler/but-ui-react/TextLink.tsx";
import {
	isAdoptionPage,
	isDesignPage,
	pageName,
	pages,
	type Adoption,
	type Page,
} from "./pages.ts";
import { pageLink } from "./site.ts";
import type { FC } from "react";

type Usage = { files: number; uses: number; names: Array<string> };

const components = pages.filter((page) => !isDesignPage(page) && !isAdoptionPage(page));

/** How many pages draw from each of the library's files. */
const modules = components.flatMap((page) => page.module ?? []);
const pagesByModule = new Map(
	modules.map((module) => [module, modules.filter((other) => other === module).length]),
);

/**
 * What but.dev uses of a page's component: all its file's names, or, where several pages share
 * that file (`SidebarRow.tsx`, `Popup.tsx`), the component's own name alone.
 */
function usage(page: Page, snapshot: Adoption): Usage | undefined {
	const module = page.module === undefined ? undefined : snapshot.modules[page.module];
	if (module === undefined) return undefined;
	if ((pagesByModule.get(page.module ?? "") ?? 0) > 1) {
		const named = page.component === undefined ? undefined : module.names[page.component];
		return named === undefined
			? undefined
			: { files: named.files, uses: named.uses, names: [page.component ?? ""] };
	}
	return {
		files: module.files,
		uses: Object.values(module.names).reduce((sum, { uses }) => sum + uses, 0),
		names: Object.keys(module.names),
	};
}

const ComponentRow: FC<{ page: Page; used: Usage | undefined }> = ({ page, used }) => (
	<TableRow>
		<TableCell>
			<a {...pageLink(page.id)} className="docs-site-page-link">
				{pageName(page)}
			</a>
		</TableCell>
		<TableCell align="end">{used?.files ?? 0}</TableCell>
		<TableCell align="end">{used?.uses ?? 0}</TableCell>
		<TableCell>
			{used !== undefined && (
				<span className="docs-adoption-names">
					{used.names.map((name) => (
						<Code key={name}>{name}</Code>
					))}
				</span>
			)}
		</TableCell>
		<TableCell align="end">{page.stories ?? 0}</TableCell>
		<TableCell>
			{page.figma !== undefined && (
				<TextLink href={page.figma} target="_blank" rel="noreferrer" className="docs-adoption-link">
					Frame
				</TextLink>
			)}
		</TableCell>
		<TableCell>{page.gpui === true && "Yes"}</TableCell>
	</TableRow>
);

const ComponentTable: FC<{ rows: Array<[Page, Usage | undefined]> }> = ({ rows }) => (
	<Table
		header={
			<TableRow>
				<TableCell header>Component</TableCell>
				<TableCell header align="end">
					Files
				</TableCell>
				<TableCell header align="end">
					Uses
				</TableCell>
				<TableCell header>Imports</TableCell>
				<TableCell header align="end">
					Stories
				</TableCell>
				<TableCell header>Figma</TableCell>
				<TableCell header>GPUI</TableCell>
			</TableRow>
		}
	>
		{rows.map(([page, used]) => (
			<ComponentRow key={page.id} page={page} used={used} />
		))}
	</Table>
);

const short = (commit: string) => commit.slice(0, 7);

/**
 * What but.dev uses of the library: every component's page with how many of but.dev's files
 * import it and how many times they name it, its stories, its frame in Figma and whether the GPUI
 * client draws it, the used ones first, the most used first. `snapshot` is `adoption.json`, which
 * `scripts/adoption.mjs` reads off a but-dev commit.
 */
export const AdoptionPage: FC<{ snapshot: Adoption }> = ({ snapshot }) => {
	const rows = components.map((page): [Page, Usage | undefined] => [page, usage(page, snapshot)]);
	const used = rows
		.filter(([, use]) => use !== undefined)
		.sort(([a, x], [b, y]) =>
			x?.files === y?.files ? a.title.localeCompare(b.title) : (y?.files ?? 0) - (x?.files ?? 0),
		);
	const unused = rows.filter(([, use]) => use === undefined);
	const drawn = new Set(components.flatMap((page) => page.module ?? []));
	const others = Object.entries(snapshot.modules).filter(([module]) => !drawn.has(module));

	return (
		<div className="docs-page">
			<header className="docs-page-header">
				<h1 className="docs-page-title text-h1">Adoption</h1>
			</header>
			<p className="docs-page-prose text-13 text-body">
				What but.dev uses of the library, read off its main branch. Files is how many of but.dev’s
				files import the component, Uses how many times they name it there. Run{" "}
				<Code>pnpm -F @gitbutler/but-ui-pantry adoption &lt;but-dev checkout&gt;</Code> to read it
				again.
			</p>
			<DescriptionList>
				<DescriptionListItem label="Components in but.dev">
					{used.length} of {components.length}
				</DescriptionListItem>
				<DescriptionListItem label="but.dev files using the library">
					{snapshot.files}
				</DescriptionListItem>
				<DescriptionListItem label="Read from">
					<Code>{short(snapshot.commit)}</Code>, {new Date(snapshot.date).toLocaleDateString()}
				</DescriptionListItem>
				{snapshot.pin !== null && (
					<DescriptionListItem label="Library pinned at">
						<Code>{short(snapshot.pin)}</Code>
						{snapshot.behind !== null &&
							(snapshot.behind === 0
								? ", up to date with master"
								: `, ${snapshot.behind} library ${snapshot.behind === 1 ? "commit" : "commits"} behind master`)}
					</DescriptionListItem>
				)}
			</DescriptionList>
			<section className="docs-adoption-section">
				<h2 className="docs-page-prose text-h2">In but.dev</h2>
				<ComponentTable rows={used} />
			</section>
			{unused.length > 0 && (
				<section className="docs-adoption-section">
					<h2 className="docs-page-prose text-h2">Not in but.dev yet</h2>
					<ComponentTable rows={unused} />
				</section>
			)}
			{others.length > 0 && (
				<section className="docs-adoption-section">
					<h2 className="docs-page-prose text-h2">Other files it imports</h2>
					<p className="docs-page-prose text-13 text-body">
						The library’s files without a page of their own: helpers, types and icons.
					</p>
					<Table
						header={
							<TableRow>
								<TableCell header>File</TableCell>
								<TableCell header align="end">
									Files
								</TableCell>
								<TableCell header>Imports</TableCell>
							</TableRow>
						}
					>
						{others.map(([module, { files, names }]) => (
							<TableRow key={module}>
								<TableCell>
									<Code>{module}</Code>
								</TableCell>
								<TableCell align="end">{files}</TableCell>
								<TableCell>
									<span className="docs-adoption-names">
										{Object.keys(names).map((name) => (
											<Code key={name}>{name}</Code>
										))}
									</span>
								</TableCell>
							</TableRow>
						))}
					</Table>
				</section>
			)}
		</div>
	);
};
