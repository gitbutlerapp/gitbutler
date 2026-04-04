import { Dialog } from "@base-ui/react";
import { FC, ReactNode, use, useState } from "react";
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import { ShortcutButton } from "#ui/ShortcutButton.tsx";
import {
	isPreviewPanelVisible,
	Panel as PanelType,
	WorkspaceLayoutContext,
} from "#ui/state/WorkspaceLayout.tsx";
import { ShortcutsBarPortalContext } from "#ui/routes/project/$id/-ShortcutsBar.tsx";
import { assert } from "#ui/routes/project/$id/-shared.tsx";
import uiStyles from "#ui/ui.module.css";
import { closeFullscreenPreviewBinding } from "./workspace/-WorkspaceShortcuts.ts";
import sharedStyles from "./-shared.module.css";

export const ProjectPreviewLayout: FC<{
	projectId: string;
	children: ReactNode;
	preview: ReactNode | null;
}> = ({ children, projectId, preview }) => {
	const [layoutState, dispatchLayout] = assert(use(WorkspaceLayoutContext));
	const inheritedShortcutsBarPortalNode = use(ShortcutsBarPortalContext);
	const [dialogShortcutsBarPortalNode, setDialogShortcutsBarPortalNode] =
		useState<HTMLElement | null>(null);
	const panelIds: Array<PanelType> = isPreviewPanelVisible(layoutState)
		? ["primary", "preview"]
		: ["primary"];
	const { defaultLayout, onLayoutChanged } = useDefaultLayout({
		id: `project:${projectId}:layout`,
		panelIds,
	});

	return (
		<ShortcutsBarPortalContext
			value={
				layoutState.isFullscreenPreviewOpen
					? (dialogShortcutsBarPortalNode ?? inheritedShortcutsBarPortalNode)
					: inheritedShortcutsBarPortalNode
			}
		>
			<Group
				className={sharedStyles.pageWithPreview}
				defaultLayout={defaultLayout}
				onLayoutChange={onLayoutChanged}
			>
				<Panel id={"primary" satisfies PanelType} minSize={500}>
					<div className={sharedStyles.primaryPanel}>{children}</div>
				</Panel>
				{isPreviewPanelVisible(layoutState) && (
					<>
						<Separator className={sharedStyles.previewResizeHandle} />
						<Panel
							id={"preview" satisfies PanelType}
							minSize={300}
							defaultSize="30%"
							className={sharedStyles.previewPanel}
						>
							{
								// There can only be one user of the ref at a time.
								layoutState.isFullscreenPreviewOpen ? null : preview
							}
						</Panel>
					</>
				)}
			</Group>
			{layoutState.isFullscreenPreviewOpen && (
				<Dialog.Root
					open
					onOpenChange={(open) => {
						dispatchLayout({
							_tag: open ? "OpenFullscreenPreview" : "CloseFullscreenPreview",
						});
					}}
				>
					<Dialog.Portal>
						<Dialog.Popup aria-label="Preview" className={sharedStyles.previewDialogPopup}>
							<div className={sharedStyles.previewDialogBody}>
								<ShortcutButton
									binding={closeFullscreenPreviewBinding}
									type="button"
									className={uiStyles.button}
									onClick={() => dispatchLayout({ _tag: "CloseFullscreenPreview" })}
								>
									{closeFullscreenPreviewBinding.description}
								</ShortcutButton>
								{preview}
							</div>
							<footer ref={setDialogShortcutsBarPortalNode} />
						</Dialog.Popup>
					</Dialog.Portal>
				</Dialog.Root>
			)}
		</ShortcutsBarPortalContext>
	);
};
