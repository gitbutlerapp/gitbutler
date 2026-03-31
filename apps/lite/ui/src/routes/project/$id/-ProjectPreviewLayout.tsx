import { Dialog } from "@base-ui/react";
import { FC, ReactNode, use, useState } from "react";
import { Group, Panel, Separator, useDefaultLayout } from "react-resizable-panels";
import { ShortcutsBarPortalContext } from "#ui/routes/project/$id/-ShortcutsBar.tsx";
import { useFullscreenPreview } from "#ui/hooks/useFullscreenPreview.ts";
import { usePreviewPanel } from "#ui/hooks/usePreviewPanel.ts";
import { bindingButtonLabel } from "#ui/shortcuts.ts";
import uiStyles from "#ui/ui.module.css";
import { closeFullscreenPreviewBinding } from "./workspace/-WorkspaceShortcuts.ts";
import sharedStyles from "./-shared.module.css";

export const ProjectPreviewLayout: FC<{
	projectId: string;
	children: ReactNode;
	preview: ReactNode | null;
}> = ({ children, projectId, preview }) => {
	const [showPreviewPanel] = usePreviewPanel();
	const [showFullscreenPreview, setShowFullscreenPreview] = useFullscreenPreview(projectId);
	const inheritedShortcutsBarPortalNode = use(ShortcutsBarPortalContext);
	const [dialogShortcutsBarPortalNode, setDialogShortcutsBarPortalNode] =
		useState<HTMLElement | null>(null);
	const { defaultLayout, onLayoutChanged } = useDefaultLayout({
		id: `project:${projectId}:layout`,
		panelIds: showPreviewPanel ? ["primary", "preview"] : ["primary"],
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
					<div className={sharedStyles.primaryPane}>{children}</div>
				</Panel>
				{showPreviewPanel && (
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
							<button
								type="button"
								className={uiStyles.button}
								onClick={() => setShowFullscreenPreview(false)}
							>
								{bindingButtonLabel(closeFullscreenPreviewBinding)}
							</button>
							{preview}
						</div>
						<footer ref={setDialogShortcutsBarPortalNode} />
					</Dialog.Popup>
				</Dialog.Portal>
			</Dialog.Root>
		</ShortcutsBarPortalContext>
	);
};
