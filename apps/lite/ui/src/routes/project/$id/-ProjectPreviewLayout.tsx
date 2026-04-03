import { Dialog } from "@base-ui/react";
import { FC, ReactNode, use, useState } from "react";
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import { classes } from "#ui/classes.ts";
import { ShortcutButton } from "#ui/ShortcutButton.tsx";
import { ShortcutsBarPortalContext } from "#ui/routes/project/$id/-ShortcutsBar.tsx";
import { useFullscreenPreview } from "#ui/hooks/useFullscreenPreview.ts";
import { usePreviewPanel } from "#ui/hooks/usePreviewPanel.ts";
import uiStyles from "#ui/ui.module.css";
import { closeFullscreenPreviewBinding } from "./workspace/-WorkspaceShortcuts.ts";
import sharedStyles from "./-shared.module.css";

export const ProjectPreviewLayout: FC<{
	projectId: string;
	children: ReactNode;
	preview: ReactNode | null;
	isSelectionInsidePreview?: boolean;
}> = ({ children, projectId, preview, isSelectionInsidePreview = false }) => {
	const [showPreviewPanel] = usePreviewPanel();
	const [showFullscreenPreview, setShowFullscreenPreview] = useFullscreenPreview(projectId);
	const shouldShowPreviewPanel = showPreviewPanel || isSelectionInsidePreview;
	const inheritedShortcutsBarPortalNode = use(ShortcutsBarPortalContext);
	const [dialogShortcutsBarPortalNode, setDialogShortcutsBarPortalNode] =
		useState<HTMLElement | null>(null);
	const { defaultLayout, onLayoutChanged } = useDefaultLayout({
		id: `project:${projectId}:layout`,
		panelIds: shouldShowPreviewPanel ? ["primary", "preview"] : ["primary"],
	});

	return (
		<ShortcutsBarPortalContext
			value={
				showFullscreenPreview
					? (dialogShortcutsBarPortalNode ?? inheritedShortcutsBarPortalNode)
					: inheritedShortcutsBarPortalNode
			}
		>
			<Group
				className={sharedStyles.pageWithPreview}
				defaultLayout={defaultLayout}
				onLayoutChange={onLayoutChanged}
			>
				<Panel id="primary" minSize={500}>
					<div
						className={classes(
							sharedStyles.primaryPane,
							isSelectionInsidePreview && sharedStyles.primaryPaneDeemphasized,
						)}
					>
						{children}
					</div>
				</Panel>
				{shouldShowPreviewPanel && (
					<>
						<Separator className={sharedStyles.previewResizeHandle} />
						<Panel
							id="preview"
							minSize={300}
							defaultSize="30%"
							className={sharedStyles.previewPane}
						>
							{preview}
						</Panel>
					</>
				)}
			</Group>
			<Dialog.Root open={showFullscreenPreview} onOpenChange={setShowFullscreenPreview}>
				<Dialog.Portal>
					<Dialog.Popup aria-label="Preview" className={sharedStyles.previewDialogPopup}>
						<div className={sharedStyles.previewDialogBody}>
							<ShortcutButton
								binding={closeFullscreenPreviewBinding}
								type="button"
								className={uiStyles.button}
								onClick={() => setShowFullscreenPreview(false)}
							>
								{closeFullscreenPreviewBinding.description}
							</ShortcutButton>
							{preview}
						</div>
						<footer ref={setDialogShortcutsBarPortalNode} />
					</Dialog.Popup>
				</Dialog.Portal>
			</Dialog.Root>
		</ShortcutsBarPortalContext>
	);
};
